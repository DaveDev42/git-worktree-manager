//! Integration tests for `gw _complete-targets`.

mod common;
use common::TestRepo;

use std::process::Command;

/// Run `gw _complete-targets` in the given directory and return the Output.
fn run_complete_targets(cwd: &std::path::Path) -> std::process::Output {
    Command::new(TestRepo::cw_bin())
        .args(["_complete-targets"])
        .current_dir(cwd)
        .output()
        .expect("failed to run gw _complete-targets")
}

/// `gw _complete-targets` should list at least the main worktree name and
/// the branch it is on.
#[test]
fn complete_targets_lists_main_worktree_name_and_branch() {
    let repo = TestRepo::new();
    let output = run_complete_targets(repo.path());

    assert!(
        output.status.success(),
        "_complete-targets exited non-zero: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.trim().is_empty(),
        "_complete-targets produced no output inside a git repo"
    );

    // The main worktree directory name is the tempdir basename.
    let dir_name = repo
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    assert!(
        stdout.lines().any(|l| l == dir_name),
        "expected worktree name '{}' in output:\n{}",
        dir_name,
        stdout
    );

    // TestRepo always creates the repo on branch "main".
    assert!(
        stdout.lines().any(|l| l == "main"),
        "expected branch 'main' in output:\n{}",
        stdout
    );
}

/// Outside a git repo `gw _complete-targets` should exit 0 with empty stdout
/// (errors are silenced, shell sees an empty completion list).
#[test]
fn complete_targets_silent_outside_repo() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let output = run_complete_targets(tmp.path());

    assert!(
        output.status.success(),
        "_complete-targets should exit 0 outside a repo, got: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.trim().is_empty(),
        "_complete-targets should produce no output outside a repo, got: {}",
        stdout
    );
}

/// When the main worktree directory name equals the branch name (e.g., a
/// repo dir named "main" on branch "main"), the BTreeSet should deduplicate
/// so the token appears exactly once.
///
/// In practice TestRepo dirs have random names, so name ≠ branch.
/// We validate the invariant by asserting the total unique-line count equals
/// `lines().count()` (i.e. the output has no duplicate lines).
#[test]
fn complete_targets_dedupes_when_name_equals_branch() {
    let repo = TestRepo::new();
    let output = run_complete_targets(repo.path());

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    let unique_count = {
        let mut set = std::collections::HashSet::new();
        lines.iter().filter(|l| set.insert(*l)).count()
    };
    assert_eq!(
        lines.len(),
        unique_count,
        "_complete-targets output contains duplicate lines:\n{}",
        stdout
    );
}
