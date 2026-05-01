//! Integration tests for `gw ls` — plain TSV output.

mod common;
use common::TestRepo;

use assert_cmd::Command;
use predicates::prelude::*;
use std::process::Command as StdCommand;

fn gw() -> Command {
    Command::cargo_bin("gw").unwrap()
}

/// `gw ls` with one additional worktree emits two lines of TSV with six columns each.
#[test]
fn ls_emits_tsv_for_a_repo_with_one_worktree() {
    let repo = TestRepo::new();
    let repo_path = repo.path();

    // Create a worktree via git directly to avoid gw's AI-tool launch path.
    let wt_path = repo_path.parent().unwrap().join(format!(
        "{}-feat-a",
        repo_path.file_name().unwrap().to_str().unwrap()
    ));
    let status = StdCommand::new("git")
        .args(["worktree", "add", "-b", "feat-a", wt_path.to_str().unwrap()])
        .current_dir(repo_path)
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@test.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@test.com")
        .status()
        .expect("git worktree add");
    assert!(status.success(), "git worktree add failed");

    let output = StdCommand::new(TestRepo::cw_bin())
        .arg("ls")
        .current_dir(repo_path)
        .env("CW_LAUNCH_METHOD", "foreground")
        .output()
        .expect("gw ls");
    assert!(
        output.status.success(),
        "gw ls failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();

    // Should have exactly two lines: one for the main worktree, one for feat-a.
    assert_eq!(lines.len(), 2, "expected 2 TSV lines, got: {:?}", lines);

    for line in &lines {
        let cols: Vec<&str> = line.split('\t').collect();
        assert_eq!(
            cols.len(),
            6,
            "expected 6 tab-separated columns, got {} in line: {:?}",
            cols.len(),
            line
        );
    }

    // Last column of the worktree line should be the worktree path.
    let wt_line = lines
        .iter()
        .find(|l| l.contains("feat-a"))
        .expect("feat-a line");
    let last_col = wt_line.split('\t').next_back().unwrap();
    assert!(
        last_col.starts_with('/'),
        "last column should be an absolute path, got: {:?}",
        last_col
    );
}

/// `gw ls` with only the main worktree emits exactly one TSV line.
#[test]
fn ls_silent_when_no_worktrees() {
    let repo = TestRepo::new();

    let output = StdCommand::new(TestRepo::cw_bin())
        .arg("ls")
        .current_dir(repo.path())
        .env("CW_LAUNCH_METHOD", "foreground")
        .output()
        .expect("gw ls");
    assert!(
        output.status.success(),
        "gw ls failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();

    // The main worktree itself is always discovered by discover_scope.
    assert_eq!(
        lines.len(),
        1,
        "expected exactly 1 TSV line for main worktree, got: {:?}",
        lines
    );
}

/// `gw ls` output must not contain ANSI escape codes even without `NO_COLOR`.
#[test]
fn ls_no_color_codes() {
    let repo = TestRepo::new();

    gw().arg("ls")
        .current_dir(repo.path())
        .env("CW_LAUNCH_METHOD", "foreground")
        .env("TERM", "dumb")
        .env("NO_COLOR", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("\x1b[").not());
}

/// `gw ls --no-cache` must fail (unrecognized argument).
/// `gw list --no-cache` must also fail (flag was removed).
#[test]
fn ls_no_no_cache_flag() {
    let repo = TestRepo::new();

    // gw ls --no-cache should be rejected
    gw().args(["ls", "--no-cache"])
        .current_dir(repo.path())
        .env("CW_LAUNCH_METHOD", "foreground")
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("unexpected argument")
                .or(predicate::str::contains("--no-cache"))
                .or(predicate::str::contains("unrecognized")),
        );

    // gw list --no-cache should also be rejected
    gw().args(["list", "--no-cache"])
        .current_dir(repo.path())
        .env("CW_LAUNCH_METHOD", "foreground")
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("unexpected argument")
                .or(predicate::str::contains("--no-cache"))
                .or(predicate::str::contains("unrecognized")),
        );
}
