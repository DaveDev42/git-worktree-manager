//! Isolated integration test: spawn a sleep process with cwd inside a worktree
//! and verify `detect_busy` finds it via the ProcessScan source.
//!
//! This test lives in its own file so it compiles to a separate test binary.
//! That guarantees it runs in its own OS process, which means the
//! `CWD_SCAN_CACHE` OnceLock in `busy` is always freshly uninitialized when
//! this test starts. If this test were co-located with other tests in the same
//! binary (e.g. `busy_detection.rs`), a sibling test that calls `detect_busy`
//! first would populate the cache before the child process is spawned, causing
//! a spurious miss on any subsequent poll.

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod unix_only {
    use std::process::{Command, Stdio};
    use std::thread::sleep;
    use std::time::{Duration, Instant};

    use git_worktree_manager::operations::busy::{detect_busy, BusySource};
    use tempfile::TempDir;

    fn wait_for<F: FnMut() -> bool>(mut f: F) -> bool {
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if f() {
                return true;
            }
            sleep(Duration::from_millis(50));
        }
        false
    }

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

        // This binary owns its own CWD_SCAN_CACHE OnceLock, so the cache is
        // unpopulated when we start. The first detect_busy call triggers a
        // real /proc walk (Linux) or lsof (macOS). Polling here accounts for
        // the small window between spawn() and the OS registering the child's
        // cwd in /proc.
        let pid = child.id();
        let found = wait_for(|| {
            detect_busy(dir.path())
                .iter()
                .any(|i| i.pid == pid && i.source == BusySource::ProcessScan)
        });

        let _ = child.kill();
        let _ = child.wait();

        assert!(
            found,
            "expected to detect spawned child pid={} within 2s",
            pid,
        );
    }
}
