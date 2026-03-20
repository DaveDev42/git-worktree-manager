/// Shared test helpers — creates temporary git repos for integration tests.
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

/// A temporary git repository for testing.
pub struct TestRepo {
    pub dir: TempDir,
}

impl TestRepo {
    /// Create a new temporary git repo with an initial commit.
    pub fn new() -> Self {
        let dir = TempDir::new().expect("Failed to create temp dir");
        let path = dir.path();

        git(path, &["init", "-b", "main"]);
        git(path, &["config", "user.name", "Test"]);
        git(path, &["config", "user.email", "test@test.com"]);
        git(path, &["config", "commit.gpgsign", "false"]);

        // Create initial commit
        std::fs::write(path.join("README.md"), "# Test\n").unwrap();
        git(path, &["add", "."]);
        git(path, &["commit", "-m", "Initial commit"]);

        Self { dir }
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// Create a branch at the current HEAD.
    pub fn create_branch(&self, name: &str) {
        git(self.path(), &["branch", name]);
    }

    /// Add a file and commit.
    pub fn commit_file(&self, name: &str, content: &str, msg: &str) {
        std::fs::write(self.path().join(name), content).unwrap();
        git(self.path(), &["add", name]);
        git(self.path(), &["commit", "-m", msg]);
    }

    /// Run a git command in this repo directory.
    pub fn git(&self, args: &[&str]) {
        git(self.path(), args);
    }

    /// Run a git command in this repo directory and return stdout.
    pub fn git_stdout(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(self.path())
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@test.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@test.com")
            .output()
            .expect("Failed to run git");
        String::from_utf8_lossy(&output.stdout).to_string()
    }

    /// Run a git command at a specific path.
    pub fn git_at(path: &Path, args: &[&str]) {
        git(path, args);
    }

    /// Run a git command at a specific path and return stdout.
    pub fn git_stdout_at(path: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(path)
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@test.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@test.com")
            .output()
            .expect("Failed to run git");
        String::from_utf8_lossy(&output.stdout).to_string()
    }

    /// Run cw command at a specific path instead of repo root.
    pub fn cw_at(path: &Path, args: &[&str]) -> std::process::Output {
        Command::new(Self::cw_bin())
            .args(args)
            .current_dir(path)
            .output()
            .expect("Failed to run cw")
    }

    /// Run cw at a path and return stdout.
    pub fn cw_stdout_at(path: &Path, args: &[&str]) -> String {
        let output = Self::cw_at(path, args);
        String::from_utf8_lossy(&output.stdout).to_string()
    }

    /// Get the cw binary path.
    pub fn cw_bin() -> PathBuf {
        PathBuf::from(env!("CARGO_BIN_EXE_gw"))
    }

    /// Run cw command in this repo.
    pub fn cw(&self, args: &[&str]) -> std::process::Output {
        Command::new(Self::cw_bin())
            .args(args)
            .current_dir(self.path())
            .output()
            .expect("Failed to run cw")
    }

    /// Run cw and return stdout as string.
    pub fn cw_stdout(&self, args: &[&str]) -> String {
        let output = self.cw(args);
        String::from_utf8_lossy(&output.stdout).to_string()
    }

    /// Run cw and return stderr as string.
    pub fn cw_stderr(&self, args: &[&str]) -> String {
        let output = self.cw(args);
        String::from_utf8_lossy(&output.stderr).to_string()
    }

    /// Run cw and return combined stdout+stderr.
    pub fn cw_combined(&self, args: &[&str]) -> String {
        let output = self.cw(args);
        format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    }

    /// Check if cw command succeeds.
    pub fn cw_ok(&self, args: &[&str]) -> bool {
        self.cw(args).status.success()
    }

    /// Create a worktree and return its path. Panics on failure.
    pub fn create_worktree(&self, branch: &str) -> PathBuf {
        let output = self.cw(&["new", branch, "--no-term"]);
        assert!(
            output.status.success(),
            "Failed to create worktree '{}': {}{}",
            branch,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        // The default path is ../<repo>-<branch>
        let wt_path = self.path().parent().unwrap().join(format!(
            "{}-{}",
            self.path().file_name().unwrap().to_str().unwrap(),
            branch
        ));
        assert!(
            wt_path.exists(),
            "Worktree path does not exist: {:?}",
            wt_path
        );
        wt_path
    }

    /// Add a file and commit in a worktree at a given path.
    pub fn commit_file_at(path: &Path, name: &str, content: &str, msg: &str) {
        std::fs::write(path.join(name), content).unwrap();
        git(path, &["add", name]);
        git(path, &["commit", "-m", msg]);
    }

    /// Set up a bare remote and add it as "origin".
    pub fn setup_remote(&self) -> PathBuf {
        let remote_path = self.path().parent().unwrap().join("remote_repo.git");
        let output = Command::new("git")
            .args([
                "clone",
                "--bare",
                self.path().to_str().unwrap(),
                remote_path.to_str().unwrap(),
            ])
            .output()
            .expect("Failed to clone bare repo");
        assert!(output.status.success(), "Failed to create bare remote");
        git(
            self.path(),
            &["remote", "add", "origin", remote_path.to_str().unwrap()],
        );
        remote_path
    }
}

fn git(path: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(path)
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@test.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@test.com")
        .output()
        .expect("Failed to run git");
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}
