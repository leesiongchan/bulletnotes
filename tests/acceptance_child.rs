#[path = "../examples/support/child_process.rs"]
mod child_process;
use child_process::ChildGuard;
use std::{
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
unsafe extern "C" {
    fn waitpid(pid: i32, status: *mut i32, options: i32) -> i32;
    fn kill(pid: i32, signal: i32) -> i32;
}
fn probe(pid: i32) -> i32 {
    let mut status = 0;
    loop {
        // WNOHANG=1 in the macOS SDK; the PID comes only from our own spawned Child.
        let result = unsafe { waitpid(pid, &mut status, 1) };
        if result != -1 || std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted
        {
            return result;
        }
    }
}
struct FailureCleanup(i32);
impl Drop for FailureCleanup {
    fn drop(&mut self) {
        if probe(self.0) == 0 {
            // Test-only fallback prevents a deliberately failing RED from leaving its child alive.
            unsafe {
                kill(self.0, 9);
                waitpid(self.0, std::ptr::null_mut(), 0);
            }
        }
    }
}
fn spawn(dir: &Path, nonce: &str) -> (std::process::Child, FailureCleanup) {
    let child = Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "owned_child"])
        .env("BULLETNOTES_CHILD_GUARD_DIR", dir)
        .env("BULLETNOTES_CHILD_GUARD_NONCE", nonce)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let cleanup = FailureCleanup(child.id() as i32);
    let deadline = Instant::now() + Duration::from_secs(5);
    while !dir.join("ready").exists() {
        assert!(Instant::now() < deadline, "owned child readiness deadline");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        std::fs::read_to_string(dir.join("ready")).unwrap(),
        format!("{}:{nonce}", child.id())
    );
    (child, cleanup)
}
#[test]
fn early_filesystem_and_validation_errors_terminate_and_reap_owned_children() {
    for validation_error in [false, true] {
        let dir = tempfile::Builder::new()
            .prefix("bulletnotes-child-guard-")
            .tempdir()
            .unwrap();
        let nonce = uuid::Uuid::new_v4().to_string();
        let (child, _failure_cleanup) = spawn(dir.path(), &nonce);
        let pid = child.id() as i32;
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            let mut child = ChildGuard::new(child);
            assert!(child.try_wait()?.is_none());
            if validation_error {
                return Err("invalid owned recovery registration".into());
            }
            std::fs::read(dir.path().join("missing-registration.json"))?;
            Ok(())
        })();
        assert!(result.is_err());
        assert_eq!(
            probe(pid),
            -1,
            "authority returned before owned child termination/reaping"
        );
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(10),
            "expected ECHILD after guard drop"
        );
    }
}
#[test]
fn explicit_stop_and_already_exited_child_are_reaped() {
    let dir = tempfile::Builder::new()
        .prefix("bulletnotes-child-guard-")
        .tempdir()
        .unwrap();
    let (child, _failure_cleanup) = spawn(dir.path(), &uuid::Uuid::new_v4().to_string());
    let pid = child.id() as i32;
    let mut child = ChildGuard::new(child);
    child.terminate_and_reap().unwrap();
    child.terminate_and_reap().unwrap();
    assert_eq!(probe(pid), -1);
    let mut completed = ChildGuard::new(
        Command::new(std::env::current_exe().unwrap())
            .arg("--list")
            .stdout(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while completed.try_wait().unwrap().is_none() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    completed.terminate_and_reap().unwrap();
}
#[test]
#[ignore = "nonce-owned headless process fixture, parent invokes and reaps"]
fn owned_child() {
    let dir = std::path::PathBuf::from(std::env::var_os("BULLETNOTES_CHILD_GUARD_DIR").unwrap());
    assert!(
        dir.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("bulletnotes-child-guard-")
    );
    let nonce = std::env::var("BULLETNOTES_CHILD_GUARD_NONCE").unwrap();
    std::fs::write(dir.join("ready"), format!("{}:{nonce}", std::process::id())).unwrap();
    std::thread::park_timeout(Duration::from_secs(10));
}
