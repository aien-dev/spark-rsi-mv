//! Offline security boundary regressions. All host effects stay inside private tempdirs.
use spark_rsi::isolation::BuildJail;
use spark_rsi::observe::observe_codebase;
use spark_rsi::supervisor::HostSupervisor;
use spark_rsi::verifier::InvariantVerifier;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::Path;

fn crate_at(root: &Path, source: &str, build: Option<&str>) {
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname='boundary-probe'\nversion='0.1.0'\nedition='2021'\n",
    )
    .unwrap();
    fs::write(
        root.join("Cargo.lock"),
        "version = 4\n\n[[package]]\nname = \"boundary-probe\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(root.join("src/lib.rs"), source).unwrap();
    if let Some(build) = build {
        fs::write(root.join("build.rs"), build).unwrap();
    }
}

#[test]
fn invariant_tests_cannot_write_host_files() {
    let tmp = tempfile::tempdir().unwrap();
    let marker = tmp.path().join("host-only-marker");
    let tree = tmp.path().join("candidate");
    crate_at(
        &tree,
        &format!(
            "#[test] fn confined() {{ assert!(std::fs::write({:?}, b\"escaped\").is_err()); }}",
            marker
        ),
        None,
    );
    let (compiled, error, passed, summary) = InvariantVerifier::verify_compilation_and_tests(&tree);
    assert!(!marker.exists(), "candidate test wrote a host-only file");
    assert!(
        compiled && passed,
        "isolated test did not run successfully: {error:?} {summary:?}"
    );
    assert!(
        !tree.join("target").exists(),
        "candidate source became writable build output"
    );
}

#[test]
fn observation_build_scripts_cannot_write_host_files() {
    let tmp = tempfile::tempdir().unwrap();
    let marker = tmp.path().join("observer-host-marker");
    let tree = tmp.path().join("candidate");
    crate_at(
        &tree,
        "pub fn answer() -> u8 { 7 }",
        Some(&format!(
            "fn main() {{ assert!(std::fs::write({:?}, b\"escaped\").is_err()); }}",
            marker
        )),
    );
    let observed = observe_codebase(&tree).unwrap();
    assert!(
        !marker.exists(),
        "observation ran a build script on the host"
    );
    assert!(
        observed.tests_passing,
        "isolated compilation did not succeed"
    );
    assert!(!tree.join("target").exists());
}

#[test]
fn failing_candidate_tests_are_not_accepted() {
    let tmp = tempfile::tempdir().unwrap();
    crate_at(
        tmp.path(),
        "#[test] fn negative_control() { panic!(\"intentional refusal\"); }",
        None,
    );
    let (compiled, _, passed, summary) =
        InvariantVerifier::verify_compilation_and_tests(tmp.path());
    assert!(compiled, "negative control must compile");
    assert!(
        !passed,
        "intentionally failing candidate was accepted: {summary:?}"
    );
}

#[test]
fn build_jail_does_not_inherit_service_environment() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    let output = tmp.path().join("output");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&output).unwrap();
    std::env::set_var("RSI_BUILD_SECRET_SENTINEL", "harmless-service-secret");
    let result = BuildJail::new("unused", &source, &output).execute_bwrap(&[
        "/bin/sh",
        "-c",
        "test -z \"$RSI_BUILD_SECRET_SENTINEL\" && printf scrubbed",
    ]);
    std::env::remove_var("RSI_BUILD_SECRET_SENTINEL");
    let (passed, stdout, stderr) = result.unwrap();
    assert!(
        passed && stdout == "scrubbed",
        "environment crossed build boundary: {stdout:?} {stderr:?}"
    );
}

#[test]
fn canary_cannot_read_or_write_supervisor_files() {
    let tmp = tempfile::tempdir().unwrap();
    let secret = tmp.path().join("operator-secret");
    let marker = tmp.path().join("canary-host-marker");
    fs::write(&secret, b"harmless-private-sentinel").unwrap();
    let network = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = network.local_addr().unwrap().port();
    let script = tmp.path().join("worker");
    fs::write(&script, format!("#!/bin/bash\nif cat {:?} > {:?} 2>/dev/null; then exit 41; fi\nif echo escaped > {:?} 2>/dev/null; then exit 42; fi\nprintf scratch-ok > /scratch/ok || exit 43\ntest \"$HOME\" = /tmp || exit 44\nif (echo escaped > /dev/tcp/127.0.0.1/{port}) 2>/dev/null; then exit 45; fi\nexit 0\n", secret, marker, marker)).unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    let socket = tmp.path().join("worker.sock");
    let _listener = UnixListener::bind(&socket).unwrap();
    let supervisor = HostSupervisor::new(&tmp.path().join("state"), 1024);
    let artifacts = tmp.path().join("artifacts");
    fs::create_dir_all(&artifacts).unwrap();
    let mut generation = supervisor
        .stage_generation("confined", &artifacts, "test")
        .unwrap();
    let mut child = supervisor
        .spawn_worker_process(&mut generation, &script, &[], &socket, &[])
        .unwrap();
    let status = child.wait().unwrap();
    assert!(
        !marker.exists(),
        "canary read or overwrote supervisor-owned files"
    );
    assert!(
        status.success(),
        "canary confinement probe did not execute: {status}"
    );
    assert_eq!(fs::read(&secret).unwrap(), b"harmless-private-sentinel");
}

fn descendants(pid: u32) -> Vec<u32> {
    let mut result = Vec::new();
    let Ok(children) = fs::read_to_string(format!("/proc/{pid}/task/{pid}/children")) else {
        return result;
    };
    for child in children.split_whitespace() {
        let child = child.parse::<u32>().unwrap();
        result.push(child);
        result.extend(descendants(child));
    }
    result
}

#[test]
fn stopping_canary_reaps_forked_descendants() {
    let tmp = tempfile::tempdir().unwrap();
    let script = tmp.path().join("worker");
    fs::write(&script, "#!/bin/sh\nsleep 60 &\nwait\n").unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    let socket = tmp.path().join("worker.sock");
    let _listener = UnixListener::bind(&socket).unwrap();
    let supervisor = HostSupervisor::new(&tmp.path().join("state"), 1024);
    let artifacts = tmp.path().join("artifacts");
    fs::create_dir_all(&artifacts).unwrap();
    let mut generation = supervisor
        .stage_generation("forking", &artifacts, "test")
        .unwrap();
    let mut child = supervisor
        .spawn_worker_process(&mut generation, &script, &[], &socket, &[])
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    let children = loop {
        let children = descendants(child.id());
        if children.len() >= 3 || std::time::Instant::now() >= deadline {
            break children;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    child.kill().unwrap();
    child.wait().unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    let survivors = loop {
        let survivors: Vec<u32> = children
            .iter()
            .copied()
            .filter(|pid| {
                fs::read_to_string(format!("/proc/{pid}/stat"))
                    .map(|stat| {
                        !stat
                            .rsplit_once(')')
                            .unwrap()
                            .1
                            .trim_start()
                            .starts_with('Z')
                    })
                    .unwrap_or(false)
            })
            .collect();
        if survivors.is_empty() || std::time::Instant::now() >= deadline {
            break survivors;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    // Cleanup is bounded to this test's recorded children even on a regressed host launch.
    for pid in &survivors {
        unsafe {
            libc::kill(*pid as i32, libc::SIGKILL);
        }
    }
    assert!(
        children.len() >= 3,
        "worker namespace and forked child did not start"
    );
    assert!(
        survivors.is_empty(),
        "canary descendants outlived monitor: {survivors:?}"
    );
}

#[test]
fn canary_tmpfs_is_charged_to_a_hard_memory_limit() {
    let tmp = tempfile::tempdir().unwrap();
    let script = tmp.path().join("worker");
    fs::write(&script, "#!/bin/sh\nprintf ok > /scratch/small || exit 40\nexec dd if=/dev/zero of=/scratch/fill bs=1M count=64 status=none\n").unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    let socket = tmp.path().join("worker.sock");
    let _listener = UnixListener::bind(&socket).unwrap();
    let supervisor = HostSupervisor::new(&tmp.path().join("state"), 32);
    let artifacts = tmp.path().join("artifacts");
    fs::create_dir_all(&artifacts).unwrap();
    let mut generation = supervisor
        .stage_generation("bounded", &artifacts, "test")
        .unwrap();
    let mut child = supervisor
        .spawn_worker_process(&mut generation, &script, &[], &socket, &[])
        .unwrap();
    let status = child.wait().unwrap();
    println!("tmpfs pressure canary status: {status}");
    assert_ne!(
        status.code(),
        Some(40),
        "scratch setup failed before memory pressure"
    );
    assert!(
        !status.success(),
        "worker filled tmpfs beyond its hard budget"
    );
}

#[test]
fn build_jail_tmpfs_is_charged_to_a_hard_memory_limit() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    let output = tmp.path().join("output");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&output).unwrap();
    let mut jail = BuildJail::new("unused", &source, &output);
    jail.limits.max_memory_mb = 32;
    let (passed, _, stderr) = jail.execute_bwrap(&[
        "/bin/sh", "-c",
        "printf scratch-ok > /tmp/small && exec dd if=/dev/zero of=/tmp/fill bs=1M count=64 status=none",
    ]).unwrap();
    assert!(
        !passed,
        "build process exceeded its hard memory limit: {stderr}"
    );
}
