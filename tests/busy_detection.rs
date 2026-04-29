//! Integration tests for busy detection: lockfile, TTY-aware delete, and
//! miscellaneous busy-detection scenarios.
//!
//! The `external_process_with_cwd_in_worktree_is_detected` test was moved to
//! `tests/busy_process_scan.rs` so it compiles to a separate test binary.
//! That guarantees it runs in its own OS process with a fresh `CWD_SCAN_CACHE`
//! OnceLock, preventing a sibling test from pre-populating the cache before
//! the target child process is spawned.

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod unix_only {
    use std::thread::sleep;
    use std::time::{Duration, Instant};

    use git_worktree_manager::operations::busy::detect_busy;
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

    fn nanos_suffix() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    }

    #[test]
    fn no_busy_when_worktree_empty() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".git")).unwrap();
        let infos = detect_busy(dir.path());
        assert!(infos.is_empty(), "unexpected busy: {:?}", infos);
    }

    #[test]
    fn busy_info_includes_tier_field() {
        use git_worktree_manager::operations::busy::{BusyInfo, BusySource, BusyTier};
        use std::path::PathBuf;
        let info = BusyInfo {
            pid: 1,
            cmd: "x".into(),
            cwd: PathBuf::from("/tmp"),
            source: BusySource::Lockfile,
            tier: BusyTier::Hard,
            tty: None,
            started_secs_ago: None,
        };
        assert_eq!(info.tier, BusyTier::Hard);
    }

    #[test]
    fn delete_refuses_with_lockfile_hard_tier_message() {
        // Construct a tempdir that LOOKS like a worktree (has .git dir),
        // write a lockfile naming a live child PID (so it isn't excluded by
        // the self-process-tree filter), then call detect_busy_tiered +
        // render_refusal directly. We don't drive the full delete_worktree
        // path (it requires a real registered worktree); we just verify the
        // tiered API + renderer agree on the Hard-tier message shape.
        use git_worktree_manager::operations::busy::detect_busy_tiered;
        use git_worktree_manager::operations::busy_messages::render_refusal;
        use git_worktree_manager::operations::lockfile::{LockEntry, LOCK_VERSION};
        use std::process::{Command, Stdio};

        let dir = tempfile::tempdir().unwrap();
        let git_dir = dir.path().join(".git");
        std::fs::create_dir_all(&git_dir).unwrap();
        let mut child = Command::new("sleep")
            .arg("30")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn sleep");
        let entry = LockEntry {
            version: LOCK_VERSION,
            pid: child.id(),
            started_at: 0,
            cmd: "claude".into(),
        };
        std::fs::write(
            git_dir.join("gw-session.lock"),
            serde_json::to_string(&entry).unwrap(),
        )
        .unwrap();

        let (hard, soft) = detect_busy_tiered(dir.path());
        let _ = child.kill();
        let _ = child.wait();
        assert!(!hard.is_empty(), "lockfile should appear as hard");
        let msg = render_refusal("feature-x", &hard, &soft);
        assert!(
            msg.contains("Cannot delete"),
            "expected hard-tier refusal phrasing, got: {msg}"
        );
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
