use std::path::{Path, PathBuf};
use std::process::Command;

use super::cargo_sandbox::{bounded_bwrap, Toolchain};

/// The jail PATH after the toolchain bin directories.
const JAIL_PATH_TAIL: &[&str] = &["/usr/local/cuda/bin", "/usr/local/bin", "/usr/bin", "/bin"];

/// Binds the host toolchain read-only at its own path and points `RUSTUP_HOME` / `CARGO_HOME`
/// at it, wherever it lives (see [`Toolchain`]).
fn push_toolchain(args: &mut Vec<String>, toolchain: &Toolchain) {
    for d in toolchain.bind_dirs() {
        let d = d.display().to_string();
        args.push("--ro-bind".to_string());
        args.push(d.clone());
        args.push(d);
    }
    for (k, d) in toolchain.env() {
        args.push("--setenv".to_string());
        args.push(k.to_string());
        args.push(d.display().to_string());
    }
}

#[derive(Debug, Clone)]
pub struct SandboxLimits {
    pub max_memory_mb: u64,
    pub max_cpu_percent: u32,
    pub max_pids: u32,
    pub timeout_seconds: u64,
}

impl Default for SandboxLimits {
    fn default() -> Self {
        Self {
            max_memory_mb: 16384,
            max_cpu_percent: 800,
            max_pids: 256,
            timeout_seconds: 180,
        }
    }
}

impl SandboxLimits {
    pub fn new(
        max_memory_mb: u64,
        max_cpu_percent: u32,
        max_pids: u32,
        timeout_seconds: u64,
    ) -> Self {
        Self {
            max_memory_mb,
            max_cpu_percent,
            max_pids,
            timeout_seconds,
        }
    }
}

fn execute_with_timeout(
    mut cmd: Command,
    timeout_secs: u64,
) -> Result<(bool, String, String), String> {
    use std::io::Read;
    use std::time::{Duration, Instant};

    let mut child = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn process: {}", e))?;

    let start = Instant::now();
    let timeout = Duration::from_secs(timeout_secs);
    let mut poll_interval = Duration::from_micros(100);

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut stdout = String::new();
                let mut stderr = String::new();
                if let Some(mut out) = child.stdout.take() {
                    let _ = out.read_to_string(&mut stdout);
                }
                if let Some(mut err) = child.stderr.take() {
                    let _ = err.read_to_string(&mut stderr);
                }
                return Ok((status.success(), stdout, stderr));
            }
            Ok(None) => {
                let elapsed = start.elapsed();
                if elapsed >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("Process timed out after {} seconds", timeout_secs));
                }
                if elapsed > Duration::from_millis(50) && poll_interval < Duration::from_millis(1) {
                    poll_interval = Duration::from_millis(1);
                } else if elapsed > Duration::from_millis(500) {
                    poll_interval = Duration::from_millis(5);
                }
                std::thread::sleep(poll_interval);
            }
            Err(e) => {
                let _ = child.kill();
                return Err(format!("Error waiting on child process: {}", e));
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct BuildJail {
    pub builder_image: String,
    pub host_source_dir: PathBuf,
    pub host_output_dir: PathBuf,
    pub limits: SandboxLimits,
}

impl BuildJail {
    pub fn new(builder_image: &str, host_source: &Path, host_output: &Path) -> Self {
        Self {
            builder_image: builder_image.to_string(),
            host_source_dir: host_source.to_path_buf(),
            host_output_dir: host_output.to_path_buf(),
            limits: SandboxLimits::default(),
        }
    }

    pub fn build_docker_args(&self, command: &[&str]) -> Vec<String> {
        let uid = unsafe { libc::getuid() }.to_string();
        let gid = unsafe { libc::getgid() }.to_string();

        let mut args = vec![
            "run".to_string(),
            "--rm".to_string(),
            "--network".to_string(),
            "none".to_string(),
            "--read-only".to_string(),
            "--cap-drop".to_string(),
            "ALL".to_string(),
            "--security-opt".to_string(),
            "no-new-privileges".to_string(),
            "--memory".to_string(),
            format!("{}m", self.limits.max_memory_mb),
            "--cpus".to_string(),
            format!("{:.2}", self.limits.max_cpu_percent as f64 / 100.0),
            "--pids-limit".to_string(),
            self.limits.max_pids.to_string(),
            "-u".to_string(),
            format!("{}:{}", uid, gid),
            "-v".to_string(),
            format!("{}:/workspace/src:ro", self.host_source_dir.display()),
            "-v".to_string(),
            format!("{}:/output:rw", self.host_output_dir.display()),
            "--tmpfs".to_string(),
            "/tmp:rw,size=4G,mode=1777".to_string(),
            "--tmpfs".to_string(),
            "/build:rw,size=8G,mode=1777".to_string(),
            "-w".to_string(),
            "/build".to_string(),
            self.builder_image.clone(),
        ];

        for c in command {
            args.push(c.to_string());
        }

        args
    }

    pub fn build_bwrap_args(&self, command: &[&str]) -> Vec<String> {
        let mut args = vec![
            "--unshare-user".to_string(),
            "--unshare-ipc".to_string(),
            "--unshare-pid".to_string(),
            "--unshare-net".to_string(),
            "--unshare-uts".to_string(),
            "--die-with-parent".to_string(),
            "--clearenv".to_string(),
            "--ro-bind".to_string(),
            "/usr".to_string(),
            "/usr".to_string(),
            "--ro-bind".to_string(),
            "/lib".to_string(),
            "/lib".to_string(),
            "--proc".to_string(),
            "/proc".to_string(),
            "--dev".to_string(),
            "/dev".to_string(),
            "--tmpfs".to_string(),
            "/tmp".to_string(),
            "--ro-bind".to_string(),
            self.host_source_dir.display().to_string(),
            "/workspace/src".to_string(),
            "--bind".to_string(),
            self.host_output_dir.display().to_string(),
            "/output".to_string(),
            "--chdir".to_string(),
            "/workspace/src".to_string(),
        ];

        if Path::new("/lib64").exists() {
            args.push("--ro-bind".to_string());
            args.push("/lib64".to_string());
            args.push("/lib64".to_string());
        }
        if Path::new("/bin").exists() {
            args.push("--ro-bind".to_string());
            args.push("/bin".to_string());
            args.push("/bin".to_string());
        }
        if Path::new("/etc").exists() {
            args.push("--ro-bind".to_string());
            args.push("/etc".to_string());
            args.push("/etc".to_string());
        }
        // Resolve the host toolchain before the launcher drops its inherited environment.
        // The only writable build output is this jail's caller-provided output directory.
        let toolchain = Toolchain::from_env();
        push_toolchain(&mut args, &toolchain);
        args.push("--setenv".to_string());
        args.push("CARGO_TARGET_DIR".to_string());
        args.push("/output/target".to_string());

        args.push("--setenv".to_string());
        args.push("PATH".to_string());
        args.push(toolchain.jail_path(JAIL_PATH_TAIL));
        args.push("--setenv".to_string());
        args.push("HOME".to_string());
        args.push("/tmp".to_string());
        args.push("--setenv".to_string());
        args.push("CARGO_BUILD_JOBS".to_string());
        args.push("2".to_string());

        for c in command {
            args.push(c.to_string());
        }

        args
    }

    pub fn execute(&self, command: &[&str]) -> Result<(bool, String, String), String> {
        let args = self.build_docker_args(command);
        let mut cmd = Command::new("docker");
        cmd.args(&args);
        execute_with_timeout(cmd, self.limits.timeout_seconds)
    }

    pub fn execute_bwrap(&self, command: &[&str]) -> Result<(bool, String, String), String> {
        // A writable alias of the source would defeat its read-only bind.
        let source = self
            .host_source_dir
            .canonicalize()
            .map_err(|e| format!("cannot resolve build source: {e}"))?;
        let output = self
            .host_output_dir
            .canonicalize()
            .map_err(|e| format!("cannot resolve build output: {e}"))?;
        if source.starts_with(&output) || output.starts_with(&source) {
            return Err("build source and output must be separate directories".to_string());
        }
        let args = self.build_bwrap_args(command);
        let mut cmd = bounded_bwrap(&self.limits, Some(self.limits.timeout_seconds));
        cmd.args(&args);
        // No retry without the network namespace: the jailed program controls its own stderr, so
        // an error-string fallback would let it ask for the network.
        execute_with_timeout(cmd, self.limits.timeout_seconds)
    }
}

#[derive(Debug, Clone)]
pub struct GpuEvaluationJail {
    pub eval_image: String,
    pub host_artifact_dir: PathBuf,
    pub cdi_device: String,
    pub limits: SandboxLimits,
}

impl GpuEvaluationJail {
    pub fn new(eval_image: &str, artifact_dir: &Path) -> Self {
        Self {
            eval_image: eval_image.to_string(),
            host_artifact_dir: artifact_dir.to_path_buf(),
            cdi_device: "nvidia.com/gpu=0".to_string(),
            limits: SandboxLimits::new(49152, 800, 256, 300), // 48 GB limit, 300s timeout
        }
    }

    pub fn build_docker_args(&self, command: &[&str]) -> Vec<String> {
        let uid = unsafe { libc::getuid() }.to_string();
        let gid = unsafe { libc::getgid() }.to_string();

        let mut args = vec![
            "run".to_string(),
            "--rm".to_string(),
            "--network".to_string(),
            "none".to_string(),
            "--device".to_string(),
            self.cdi_device.clone(),
            "--read-only".to_string(),
            "--cap-drop".to_string(),
            "ALL".to_string(),
            "--security-opt".to_string(),
            "no-new-privileges".to_string(),
            "--memory".to_string(),
            format!("{}m", self.limits.max_memory_mb),
            "--cpus".to_string(),
            format!("{:.2}", self.limits.max_cpu_percent as f64 / 100.0),
            "--pids-limit".to_string(),
            self.limits.max_pids.to_string(),
            "-u".to_string(),
            format!("{}:{}", uid, gid),
            "-v".to_string(),
            format!("{}:/artifacts:ro", self.host_artifact_dir.display()),
            "--tmpfs".to_string(),
            "/tmp:rw,size=4G,mode=1777".to_string(),
            "-w".to_string(),
            "/artifacts".to_string(),
            self.eval_image.clone(),
        ];

        for c in command {
            args.push(c.to_string());
        }

        args
    }

    pub fn execute(&self, command: &[&str]) -> Result<(bool, String, String), String> {
        let args = self.build_docker_args(command);
        let mut cmd = Command::new("docker");
        cmd.args(&args);
        execute_with_timeout(cmd, self.limits.timeout_seconds)
    }
}

#[derive(Debug, Clone)]
pub struct CandidateJailRunner {
    pub candidate_bin: PathBuf,
    pub working_dir: Option<PathBuf>,
    pub limits: SandboxLimits,
}

impl CandidateJailRunner {
    pub fn new(candidate_bin: &Path) -> Self {
        Self {
            candidate_bin: candidate_bin.to_path_buf(),
            working_dir: None,
            limits: SandboxLimits::new(49152, 800, 256, 30),
        }
    }

    pub fn with_working_dir(mut self, dir: &Path) -> Self {
        self.working_dir = Some(dir.to_path_buf());
        self
    }

    pub fn with_timeout_seconds(mut self, timeout_seconds: u64) -> Self {
        self.limits.timeout_seconds = timeout_seconds;
        self
    }

    pub fn build_bwrap_args(&self, command_args: &[&str]) -> Vec<String> {
        let mut args = vec![
            "--unshare-user".to_string(),
            "--unshare-ipc".to_string(),
            "--unshare-pid".to_string(),
            "--unshare-net".to_string(),
            "--unshare-uts".to_string(),
            "--die-with-parent".to_string(),
            // The candidate never sees the judge's environment (a key passed by variable included).
            "--clearenv".to_string(),
            "--ro-bind".to_string(),
            "/usr".to_string(),
            "/usr".to_string(),
            "--ro-bind".to_string(),
            "/lib".to_string(),
            "/lib".to_string(),
            "--proc".to_string(),
            "/proc".to_string(),
            "--dev".to_string(),
            "/dev".to_string(),
            "--tmpfs".to_string(),
            "/tmp".to_string(),
        ];

        if Path::new("/lib64").exists() {
            args.push("--ro-bind".to_string());
            args.push("/lib64".to_string());
            args.push("/lib64".to_string());
        }
        if Path::new("/bin").exists() {
            args.push("--ro-bind".to_string());
            args.push("/bin".to_string());
            args.push("/bin".to_string());
        }
        if Path::new("/etc").exists() {
            args.push("--ro-bind".to_string());
            args.push("/etc".to_string());
            args.push("/etc".to_string());
        }
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let toolchain = Toolchain::from_env();
        push_toolchain(&mut args, &toolchain);

        let bin_dir = self
            .candidate_bin
            .parent()
            .unwrap_or_else(|| Path::new("/"));
        args.push("--ro-bind".to_string());
        args.push(bin_dir.display().to_string());
        args.push("/app".to_string());

        let bin_name = self
            .candidate_bin
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("candidate");
        let in_jail_bin = format!("/app/{}", bin_name);

        if let Some(ref work) = self.working_dir {
            args.push("--bind".to_string());
            args.push(work.display().to_string());
            args.push("/workspace".to_string());
            args.push("--chdir".to_string());
            args.push("/workspace".to_string());
        } else {
            args.push("--chdir".to_string());
            args.push("/tmp".to_string());
        }

        args.push("--setenv".to_string());
        args.push("PATH".to_string());
        args.push(toolchain.jail_path(JAIL_PATH_TAIL));
        args.push("--setenv".to_string());
        args.push("HOME".to_string());
        args.push(home.clone());

        args.push(in_jail_bin);
        for arg in command_args {
            args.push(arg.to_string());
        }

        args
    }

    pub fn execute(&self, command_args: &[&str]) -> Result<(bool, String, String), String> {
        let args = self.build_bwrap_args(command_args);
        let mut cmd = Command::new("bwrap");
        cmd.args(&args);
        // No retry without the network namespace: the jailed program controls its own stderr, so
        // an error-string fallback would let it ask for the network.
        execute_with_timeout(cmd, self.limits.timeout_seconds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_jail_docker_args() {
        let jail = BuildJail::new(
            "spark-rsi-builder@sha256:123456",
            Path::new("/tmp/src"),
            Path::new("/tmp/out"),
        );
        let args = jail.build_docker_args(&["cargo", "build", "--offline"]);

        assert!(args.contains(&"--network".to_string()));
        assert!(args.contains(&"none".to_string()));
        assert!(args.contains(&"--read-only".to_string()));
        assert!(args.contains(&"--cap-drop".to_string()));
        assert!(args.contains(&"ALL".to_string()));
        assert!(args.contains(&"--pids-limit".to_string()));
        assert!(args.contains(&"/tmp/src:/workspace/src:ro".to_string()));
        assert!(args.contains(&"/tmp/out:/output:rw".to_string()));
        assert!(args.contains(&"cargo".to_string()));
    }

    #[test]
    fn test_build_jail_bwrap_args() {
        let jail = BuildJail::new(
            "spark-rsi-builder@sha256:123456",
            Path::new("/tmp/src"),
            Path::new("/tmp/out"),
        );
        let args = jail.build_bwrap_args(&["cargo", "build"]);

        assert!(args.contains(&"--unshare-net".to_string()));
        assert!(args.contains(&"--die-with-parent".to_string()));
        assert!(args.contains(&"/output".to_string()));
        assert!(args.contains(&"cargo".to_string()));
        assert!(args.contains(&"--clearenv".to_string()));
        let clear = args.iter().position(|arg| arg == "--clearenv").unwrap();
        let set = args.iter().position(|arg| arg == "--setenv").unwrap();
        assert!(clear < set);
        assert!(args
            .windows(3)
            .any(|w| w == ["--ro-bind", "/tmp/src", "/workspace/src"]));
        assert!(args
            .windows(3)
            .any(|w| w == ["--bind", "/tmp/out", "/output"]));
        assert!(args.windows(2).any(|w| w == ["--chdir", "/workspace/src"]));
        assert!(args.windows(3).any(|w| w == ["--setenv", "HOME", "/tmp"]));
        assert!(args
            .windows(3)
            .any(|w| w == ["--setenv", "CARGO_TARGET_DIR", "/output/target"]));
        assert!(args
            .windows(3)
            .any(|w| w == ["--setenv", "CARGO_BUILD_JOBS", "2"]));
        assert!(!args.iter().any(|arg| arg.contains("/tmp/spark-rsi-target")));
        // The toolchain the host resolves is bound read-only at its own path and named by its
        // variables, wherever it lives.
        let toolchain = Toolchain::from_env();
        for d in toolchain.bind_dirs() {
            let d = d.display().to_string();
            assert!(
                args.windows(3)
                    .any(|w| w[0] == "--ro-bind" && w[1] == d && w[2] == d),
                "toolchain dir {} not bound",
                d
            );
        }
        for (k, d) in toolchain.env() {
            let d = d.display().to_string();
            assert!(args
                .windows(3)
                .any(|w| w[0] == "--setenv" && w[1] == k && w[2] == d));
        }
    }

    #[test]
    fn test_build_jail_refuses_writable_source_alias() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let output = source.join("output");
        std::fs::create_dir_all(&output).unwrap();
        let jail = BuildJail::new("unused", &source, &output);
        let refusal = jail.execute_bwrap(&["cargo", "build"]).unwrap_err();
        assert!(refusal.contains("separate directories"));
    }

    #[test]
    fn test_gpu_eval_jail_args_network_none_and_no_holdouts() {
        let jail = GpuEvaluationJail::new("spark-rsi-eval@sha256:abcdef", Path::new("/tmp/art"));
        let args = jail.build_docker_args(&["./bench_suite", "--iterations", "30"]);

        assert!(args.contains(&"--network".to_string()));
        assert!(args.contains(&"none".to_string()));
        assert!(args.contains(&"--device".to_string()));
        assert!(args.contains(&"nvidia.com/gpu=0".to_string()));
        assert!(args.contains(&"--pids-limit".to_string()));
        assert!(args.contains(&"256".to_string()));
        assert!(args.contains(&"/tmp/art:/artifacts:ro".to_string()));
        // CRITICAL INVARIANT: Candidate jail must NEVER mount holdouts
        for arg in &args {
            assert!(
                !arg.contains("holdout"),
                "Candidate jail mounted holdout path: {}",
                arg
            );
        }
        assert!(args.contains(&"30".to_string()));
    }
}
