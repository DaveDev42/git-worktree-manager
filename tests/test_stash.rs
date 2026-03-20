/// Tests for stash operations.
/// Ported from tests/test_stash.py (10 tests).
mod common;

use common::TestRepo;

#[test]
fn test_stash_save_no_changes() {
    let repo = TestRepo::new();
    let stdout = repo.cw_stdout(&["stash", "save"]);
    assert!(stdout.contains("No changes to stash") || stdout.contains("No stashes"));
}

#[test]
fn test_stash_save_with_message() {
    let repo = TestRepo::new();
    std::fs::write(repo.path().join("dirty.txt"), "dirty").unwrap();
    let output = repo.cw(&["stash", "save", "my work in progress"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Stashed") || stdout.contains("stash"));
}

#[test]
fn test_stash_save_without_message() {
    let repo = TestRepo::new();
    std::fs::write(repo.path().join("dirty.txt"), "dirty").unwrap();
    let output = repo.cw(&["stash", "save"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("WIP") || stdout.contains("Stashed"));
}

#[test]
fn test_stash_list_empty() {
    let repo = TestRepo::new();
    let stdout = repo.cw_stdout(&["stash", "list"]);
    assert!(stdout.contains("No stashes found"));
}

#[test]
fn test_stash_list_after_save() {
    let repo = TestRepo::new();
    std::fs::write(repo.path().join("dirty.txt"), "dirty").unwrap();
    repo.cw(&["stash", "save", "test stash"]);

    let stdout = repo.cw_stdout(&["stash", "list"]);
    assert!(stdout.contains("main") || stdout.contains("stash"));
}

#[test]
fn test_stash_apply_nonexistent_branch() {
    let repo = TestRepo::new();
    let output = repo.cw(&["stash", "apply", "nonexistent-branch"]);
    assert!(!output.status.success());
}

#[test]
fn test_stash_apply_invalid_stash_ref() {
    let repo = TestRepo::new();
    // Create a worktree first
    repo.cw(&["new", "test-branch", "--no-term"]);
    let output = repo.cw(&["stash", "apply", "test-branch", "--stash", "stash@{99}"]);
    assert!(!output.status.success());
}
