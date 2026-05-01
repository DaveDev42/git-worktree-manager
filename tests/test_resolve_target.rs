//! Tests for `resolve_target_strict` — exact name → branch → path resolution.

mod common;
use common::TestRepo;

use git_worktree_manager::error::CwError;
use git_worktree_manager::operations::helpers::resolve_target_strict;
use std::process::Command;

#[test]
fn resolve_strict_exact_worktree_name_wins() {
    let repo = TestRepo::new();
    let wt_path = repo.create_worktree("feat-x");

    // The worktree directory basename follows the <repo-tmpname>-<branch> convention.
    // Pass the full basename to verify that rule 1 (worktree name) fires, not rule 2.
    let basename = wt_path.file_name().unwrap().to_str().unwrap();
    let result = resolve_target_strict(repo.path(), basename).unwrap();
    assert_eq!(result.branch, Some("feat-x".to_string()));
}

#[test]
fn resolve_strict_exact_branch_name_wins() {
    let repo = TestRepo::new();
    repo.create_worktree("branch-lookup");

    // "branch-lookup" matches the branch field of the worktree entry
    let result = resolve_target_strict(repo.path(), "branch-lookup").unwrap();
    assert_eq!(result.branch, Some("branch-lookup".to_string()));
    assert!(result.path.exists());
}

#[test]
fn resolve_strict_unknown_returns_err() {
    let repo = TestRepo::new();
    let result = resolve_target_strict(repo.path(), "nonexistent");
    let err = result.expect_err("Expected Err for unknown target");
    assert!(
        matches!(err, CwError::WorktreeNotFound(_)),
        "Expected CwError::WorktreeNotFound, got: {:?}",
        err
    );
}

#[test]
fn resolve_strict_exact_path_wins() {
    let repo = TestRepo::new();
    let wt_path = repo.create_worktree("path-lookup");

    // Pass the absolute path as the target.
    // Canonicalize both sides because git may resolve symlinks on macOS
    // (e.g. /var/folders/... → /private/var/folders/...).
    let result = resolve_target_strict(repo.path(), wt_path.to_str().unwrap()).unwrap();
    let result_canon = result.path.canonicalize().unwrap_or(result.path.clone());
    let expect_canon = wt_path.canonicalize().unwrap_or(wt_path.clone());
    assert_eq!(result_canon, expect_canon);
}

#[test]
fn resolve_strict_detached_worktree_yields_none_branch() {
    let repo = TestRepo::new();
    // Create a worktree, then detach it.
    let wt_path = repo.create_worktree("temp-branch");
    Command::new("git")
        .args(["checkout", "--detach"])
        .current_dir(&wt_path)
        .output()
        .expect("git checkout --detach");

    // Resolve by directory basename (rule 1 — works regardless of branch state).
    let basename = wt_path.file_name().unwrap().to_str().unwrap();
    let result = resolve_target_strict(repo.path(), basename).unwrap();
    assert!(
        result.branch.is_none(),
        "Expected None for detached, got {:?}",
        result.branch
    );
}

#[test]
fn resolve_strict_existing_path_not_a_worktree_returns_err() {
    let repo = TestRepo::new();
    // Create a temp dir that exists but is not a registered worktree.
    let tmp = tempfile::TempDir::new().unwrap();
    let result = resolve_target_strict(repo.path(), tmp.path().to_str().unwrap());
    let err = result.expect_err("Expected Err for non-worktree path");
    assert!(
        matches!(err, CwError::WorktreeNotFound(_)),
        "Expected CwError::WorktreeNotFound, got: {:?}",
        err
    );
}
