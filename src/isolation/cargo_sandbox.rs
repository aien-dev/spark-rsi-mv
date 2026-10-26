//! The one sandbox the judge uses for anything that compiles or runs candidate source.
use std::path::{Path, PathBuf};
use std::process::Command;

/// Where this host's Rust toolchain lives, resolved the way cargo and rustup resolve it:
/// `RUSTUP_HOME` and `CARGO_HOME` when set, else `$HOME/.rustup` and `$HOME/.cargo`, plus the
/// `PATH` directory that holds `cargo`. Every jail that runs cargo binds exactly these
/// directories read-only and points the same variables at them, so a host whose toolchain is
/// not under `$HOME` (the Spark judge sandbox keeps it in `/var/lib/aien-run/{cargo,rustup}`)
/// still builds. A directory that is or holds the judge's home is never exposed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Toolchain {
    pub rustup_home: Option<PathBuf>,
    pub cargo_home: Option<PathBuf>,
    pub cargo_dir: Option<PathBuf>,
}

impl Toolchain {
    /// Resolves the toolchain from this process's environment.
    pub fn from_env() -> Self {
        Self::resolve(
            std::env::var_os("HOME").map(PathBuf::from),
            std::env::var_os("RUSTUP_HOME").map(PathBuf::from),
            std::env::var_os("CARGO_HOME").map(PathBuf::from),
            &std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into()),
        )
    }

    /// Resolves the toolchain from explicit values (only directories that exist count).
    pub fn resolve(
        home: Option<PathBuf>,
        rustup_home: Option<PathBuf>,
        cargo_home: Option<PathBuf>,
        path: &str,
    ) -> Self {
        let safe = |d: &PathBuf| home.as_ref().is_none_or(|h| !h.starts_with(d));
        let pick = |set: Option<PathBuf>, default: &str| {
            set.or_else(|| home.as_ref().map(|h| h.join(default)))
                .filter(|p| p.is_dir())
                .filter(safe)
        };
        let rustup_home = pick(rustup_home, ".rustup");
        let cargo_home = pick(cargo_home, ".cargo");
        let cargo_dir = path
            .split(':')
            .map(PathBuf::from)
            .find(|d| d.join("cargo").is_file())
            .filter(safe);
        Self {
            rustup_home,
            cargo_home,
            cargo_dir,
        }
    }

    /// The directories a jail binds read-only (each once).
    pub fn bind_dirs(&self) -> Vec<&Path> {
        let mut dirs: Vec<&Path> = Vec::new();
        for d in [&self.rustup_home, &self.cargo_home, &self.cargo_dir]
            .into_iter()
            .flatten()
        {
            if !dirs.contains(&d.as_path()) {
                dirs.push(d);
            }
        }
        dirs
    }

    /// The `RUSTUP_HOME` / `CARGO_HOME` values a jail sets.
    pub fn env(&self) -> Vec<(&'static str, &Path)> {
        let mut env = Vec::new();
        if let Some(d) = &self.rustup_home {
            env.push(("RUSTUP_HOME", d.as_path()));
        }
        if let Some(d) = &self.cargo_home {
            env.push(("CARGO_HOME", d.as_path()));
        }
        env
    }

    /// The jail's `PATH`: the toolchain's bin directories first, then `tail`.
    pub fn jail_path(&self, tail: &[&str]) -> String {
        let mut path: Vec<String> = Vec::new();
        let bins = [
            self.cargo_dir.clone(),
            self.cargo_home.as_ref().map(|c| c.join("bin")),
        ];
        for d in bins.into_iter().flatten() {
            let d = d.display().to_string();
            if !path.contains(&d) {
                path.push(d);
            }
        }
        for t in tail {
            if !path.iter().any(|p| p == t) {
                path.push(t.to_string());
            }
        }
        path.join(":")
    }
}

/// The common fail-closed host launcher for untrusted code. The user scope charges
/// the whole process tree, including tmpfs/memfd, rather than sampled process RSS.
pub(crate) fn bounded_bwrap(
    limits: &super::container::SandboxLimits,
    timeout: Option<u64>,
) -> Command {
    let mut cmd = Command::new("/usr/bin/systemd-run");
    cmd.env_clear();
    // This is launcher-only: bubblewrap clears it before starting candidate code.
    cmd.env(
        "XDG_RUNTIME_DIR",
        std::env::var_os("XDG_RUNTIME_DIR")
            .unwrap_or_else(|| format!("/run/user/{}", unsafe { libc::getuid() }).into()),
    );
    cmd.args(["--user", "--scope", "--quiet", "--collect"]);
    cmd.arg(format!("--property=MemoryMax={}M", limits.max_memory_mb));
    cmd.arg("--property=MemorySwapMax=0");
    cmd.arg(format!("--property=TasksMax={}", limits.max_pids));
    cmd.arg(format!("--property=CPUQuota={}%", limits.max_cpu_percent));
    if let Some(seconds) = timeout {
        cmd.arg(format!("--property=RuntimeMaxSec={}s", seconds));
    }
    cmd.arg("/usr/bin/bwrap");
    cmd
}

/// `cargo <args>` for `tree` inside bubblewrap: no network, cleared launcher and jail
/// environments, no view of the judge's home (where its signing key lives), the tree read-only
/// at `/src`, only `target` writable (at `/target`, which must exist), and the toolchain read-only.
/// Cargo compilation is limited to two jobs. Build scripts, proc macros, `include_bytes!` and
/// the candidate's own tests run or read here, so they cannot reach the key. There is no
/// fallback: if the sandbox cannot start, the command fails.
pub fn sandboxed_cargo(tree: &Path, target: &Path, args: &[&str]) -> Command {
    let toolchain = Toolchain::from_env();
    let limits = super::container::SandboxLimits::default();
    let mut cmd = bounded_bwrap(&limits, Some(limits.timeout_seconds));
    cmd.args([
        "--unshare-user",
        "--unshare-ipc",
        "--unshare-pid",
        "--unshare-net",
        "--unshare-uts",
        "--die-with-parent",
        "--clearenv",
        "--proc",
        "/proc",
        "--dev",
        "/dev",
        "--tmpfs",
        "/tmp",
    ]);
    for sys in ["/usr", "/lib", "/lib64", "/bin", "/etc"] {
        if Path::new(sys).exists() {
            cmd.args(["--ro-bind", sys, sys]);
        }
    }
    for d in toolchain.bind_dirs() {
        cmd.arg("--ro-bind").arg(d).arg(d);
    }
    for (k, d) in toolchain.env() {
        cmd.arg("--setenv").arg(k).arg(d);
    }
    cmd.arg("--ro-bind").arg(tree).arg("/src");
    cmd.arg("--bind").arg(target).arg("/target");
    let path_in = match &toolchain.cargo_dir {
        Some(d) => format!("{}:/usr/bin:/bin", d.display()),
        None => "/usr/bin:/bin".to_string(),
    };
    cmd.args(["--setenv", "PATH", &path_in]);
    cmd.args([
        "--setenv",
        "HOME",
        "/tmp",
        "--setenv",
        "CARGO_TARGET_DIR",
        "/target",
        "--setenv",
        "CARGO_BUILD_JOBS",
        "2",
    ]);
    cmd.args(["--chdir", "/src", "cargo"]);
    cmd.args(args);
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sandbox_cargo_clears_environment_and_uses_only_isolated_output() {
        let cmd = sandboxed_cargo(
            Path::new("/tmp/candidate-source"),
            Path::new("/tmp/candidate-target"),
            &["check", "--offline", "--locked"],
        );
        let args: Vec<_> = cmd
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        let clear = args.iter().position(|arg| arg == "--clearenv").unwrap();
        let set = args.iter().position(|arg| arg == "--setenv").unwrap();
        assert!(clear < set);
        assert!(args.windows(3).any(|w| w == ["--setenv", "HOME", "/tmp"]));
        assert!(args
            .windows(3)
            .any(|w| w == ["--setenv", "CARGO_TARGET_DIR", "/target"]));
        assert!(args
            .windows(3)
            .any(|w| w == ["--setenv", "CARGO_BUILD_JOBS", "2"]));
        assert!(args
            .windows(3)
            .any(|w| w == ["--ro-bind", "/tmp/candidate-source", "/src"]));
        assert!(args
            .windows(3)
            .any(|w| w == ["--bind", "/tmp/candidate-target", "/target"]));
        assert!(args.ends_with(&["cargo", "check", "--offline", "--locked"].map(str::to_string)));
        let toolchain = Toolchain::from_env();
        for d in toolchain.bind_dirs() {
            let d = d.display().to_string();
            assert!(args
                .windows(3)
                .any(|w| w[0] == "--ro-bind" && w[1] == d && w[2] == d));
        }
        for (key, path) in toolchain.env() {
            let path = path.display().to_string();
            assert!(args
                .windows(3)
                .any(|w| w[0] == "--setenv" && w[1] == key && w[2] == path));
        }
    }

    /// The Spark judge sandbox keeps the toolchain beside HOME, not under it: HOME is
    /// `/var/lib/aien-run`, the toolchain `/var/lib/aien-run/{cargo,rustup}`. The jails must
    /// find it through CARGO_HOME / RUSTUP_HOME and PATH, not by guessing `$HOME/.cargo`.
    #[test]
    fn toolchain_outside_home_dotdirs_is_found_through_its_variables() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().to_path_buf();
        let cargo_home = home.join("cargo");
        let rustup_home = home.join("rustup");
        std::fs::create_dir_all(cargo_home.join("bin")).unwrap();
        std::fs::create_dir_all(&rustup_home).unwrap();
        std::fs::write(cargo_home.join("bin/cargo"), b"").unwrap();
        let path = format!("{}:/usr/bin:/bin", cargo_home.join("bin").display());

        let t = Toolchain::resolve(
            Some(home.clone()),
            Some(rustup_home.clone()),
            Some(cargo_home.clone()),
            &path,
        );
        assert_eq!(t.rustup_home.as_deref(), Some(rustup_home.as_path()));
        assert_eq!(t.cargo_home.as_deref(), Some(cargo_home.as_path()));
        assert_eq!(t.cargo_dir, Some(cargo_home.join("bin")));
        assert!(t.bind_dirs().contains(&cargo_home.as_path()));
        assert!(t.bind_dirs().contains(&rustup_home.as_path()));
        assert!(t
            .jail_path(&["/usr/bin", "/bin"])
            .starts_with(&cargo_home.join("bin").display().to_string()));

        // Without the variables the `$HOME/.cargo` guess finds nothing here: the old behaviour.
        let guessed = Toolchain::resolve(Some(home), None, None, "/usr/bin:/bin");
        assert!(guessed.cargo_home.is_none() && guessed.rustup_home.is_none());
    }

    #[test]
    fn a_directory_holding_the_judge_home_is_never_exposed() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("judge");
        std::fs::create_dir_all(&home).unwrap();
        let t = Toolchain::resolve(
            Some(home),
            Some(root.path().to_path_buf()),
            Some(root.path().to_path_buf()),
            "/usr/bin:/bin",
        );
        assert!(t.rustup_home.is_none());
        assert!(t.cargo_home.is_none());
        assert!(t.bind_dirs().is_empty());
    }
}
