//! Integration test: spawn a sleep process with cwd inside a worktree
//! and verify `detect_busy` finds it.

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod unix_only {
    use std::process::{Command, Stdio};
    use std::thread::sleep;
    use std::time::Duration;

    use git_worktree_manager::operations::busy::{detect_busy, BusySource};
    use tempfile::TempDir;

    #[test]
    fn external_process_with_cwd_in_worktree_is_detected() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".git")).unwrap();

        let mut child = Command::new("sleep")
            .arg("30")
            .current_dir(dir.path())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn sleep");

        sleep(Duration::from_millis(200));

        let infos = detect_busy(dir.path());
        let found = infos
            .iter()
            .any(|i| i.pid == child.id() && i.source == BusySource::ProcessScan);

        let _ = child.kill();
        let _ = child.wait();

        assert!(
            found,
            "expected to detect spawned child pid={} in {:?}",
            child.id(),
            infos
        );
    }

    #[test]
    fn no_busy_when_worktree_empty() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".git")).unwrap();
        let infos = detect_busy(dir.path());
        assert!(infos.is_empty(), "unexpected busy: {:?}", infos);
    }
}
