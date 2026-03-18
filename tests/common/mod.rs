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

    /// Get the cw binary path.
    pub fn cw_bin() -> PathBuf {
        let mut path = PathBuf::from(env!("CARGO_BIN_EXE_cw"));
        path
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

    /// Check if cw command succeeds.
    pub fn cw_ok(&self, args: &[&str]) -> bool {
        self.cw(args).status.success()
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
