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

    #[test]
    fn gw_delete_rejects_busy_worktree_when_not_tty() {
        use assert_cmd::Command;
        use std::process::{Command as StdCommand, Stdio};
        use std::thread::sleep;
        use std::time::Duration;

        let repo = tempfile::TempDir::new().unwrap();
        let init = StdCommand::new("git")
            .arg("init")
            .current_dir(repo.path())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if init.map(|s| !s.success()).unwrap_or(true) {
            return; // skip
        }

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

        let wt_path = repo.path().parent().unwrap().join("wt-busy-test-xyz");
        let _ = std::fs::remove_dir_all(&wt_path);
        let add = StdCommand::new("git")
            .args([
                "worktree",
                "add",
                "-b",
                "busy-branch-xyz",
                wt_path.to_str().unwrap(),
            ])
            .current_dir(repo.path())
            .status();
        if add.map(|s| !s.success()).unwrap_or(true) {
            return;
        }

        let mut child = StdCommand::new("sleep")
            .arg("30")
            .current_dir(&wt_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        sleep(Duration::from_millis(200));

        let output = Command::cargo_bin("gw")
            .unwrap()
            .args(["delete", "--no-force", "busy-branch-xyz"])
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
            .args(["branch", "-D", "busy-branch-xyz"])
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
