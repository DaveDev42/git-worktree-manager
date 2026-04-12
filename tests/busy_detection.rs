//! Integration test: spawn a sleep process with cwd inside a worktree
//! and verify `detect_busy` finds it.

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod unix_only {
    use std::process::{Command, Stdio};
    use std::thread::sleep;
    use std::time::{Duration, Instant};

    use git_worktree_manager::operations::busy::{detect_busy, BusySource};
    use tempfile::TempDir;

    /// Poll `f` up to ~2 seconds, 50ms between attempts. Returns true if
    /// the predicate ever fires. Avoids flaky magic-sleep timing.
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

    fn nanos_suffix() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
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

        // Note: detect_busy caches the cwd-scan. Since this is the first
        // call in this test binary for this worktree, polling is fine —
        // but in principle the cache could be populated before the child
        // registered in another test. Each #[test] runs in its own process
        // (cargo default), so we're safe.
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

    #[test]
    fn no_busy_when_worktree_empty() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".git")).unwrap();
        let infos = detect_busy(dir.path());
        assert!(infos.is_empty(), "unexpected busy: {:?}", infos);
    }

    #[test]
    fn gw_delete_rejects_busy_worktree_when_not_tty() {
        use assert_cmd::Command;
        use std::process::{Command as StdCommand, Stdio};

        let repo = tempfile::TempDir::new().unwrap();
        let init = StdCommand::new("git")
            .arg("init")
            .current_dir(repo.path())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("git init: git must be installed for this test");
        assert!(init.success(), "git init failed");

        std::fs::write(repo.path().join("README"), "hi").unwrap();
        let _ = StdCommand::new("git")
            .args(["-c", "user.email=t@t", "-c", "user.name=t", "add", "."])
            .current_dir(repo.path())
            .status();
        let _ = StdCommand::new("git")
            .args([
                "-c",
                "user.email=t@t",
                "-c",
                "user.name=t",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-m",
                "i",
            ])
            .current_dir(repo.path())
            .status();

        // Random-ish suffix to avoid collisions between parallel test runs.
        let suffix = format!("{}-{}", std::process::id(), nanos_suffix());
        let branch = format!("busy-branch-{}", suffix);
        let wt_path = repo
            .path()
            .parent()
            .unwrap()
            .join(format!("wt-busy-{}", suffix));
        let _ = std::fs::remove_dir_all(&wt_path);
        let add = StdCommand::new("git")
            .args(["worktree", "add", "-b", &branch, wt_path.to_str().unwrap()])
            .current_dir(repo.path())
            .status()
            .expect("git worktree add");
        assert!(add.success(), "git worktree add failed");

        let mut child = StdCommand::new("sleep")
            .arg("30")
            .current_dir(&wt_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();

        // Poll until detect_busy sees the child, so the subsequent
        // `gw delete` has a reliable busy signal to react to.
        let pid = child.id();
        let _ = wait_for(|| detect_busy(&wt_path).iter().any(|i| i.pid == pid));

        let output = Command::cargo_bin("gw")
            .unwrap()
            .args(["delete", &branch])
            .current_dir(repo.path())
            .write_stdin("")
            .output()
            .unwrap();

        let _ = child.kill();
        let _ = child.wait();

        // Best-effort cleanup
        let _ = StdCommand::new("git")
            .args(["worktree", "remove", "--force", wt_path.to_str().unwrap()])
            .current_dir(repo.path())
            .status();
        let _ = StdCommand::new("git")
            .args(["branch", "-D", &branch])
            .current_dir(repo.path())
            .status();

        assert!(
            !output.status.success(),
            "expected gw delete to fail for busy worktree; status: {:?}; stdout: {}; stderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("in use") || stderr.contains("busy") || stderr.contains("--force"),
            "stderr should mention busy/force: {}",
            stderr
        );
    }
}
