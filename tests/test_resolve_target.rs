//! Tests for `resolve_target_strict` — exact name → branch → path resolution.

mod common;
use common::TestRepo;

use git_worktree_manager::operations::helpers::resolve_target_strict;

#[test]
fn resolve_strict_exact_worktree_name_wins() {
    let repo = TestRepo::new();
    repo.create_worktree("feat-x");

    // The worktree directory name follows the <repo>-<branch> convention,
    // so the "name" (basename) is something like "<tmpdir>-feat-x".
    // resolve_target_strict accepts the branch name as both branch and name.
    let result = resolve_target_strict(repo.path(), "feat-x").unwrap();
    assert_eq!(result.branch, "feat-x");
}

#[test]
fn resolve_strict_exact_branch_name_wins() {
    let repo = TestRepo::new();
    repo.create_worktree("branch-lookup");

    // "branch-lookup" matches the branch field of the worktree entry
    let result = resolve_target_strict(repo.path(), "branch-lookup").unwrap();
    assert_eq!(result.branch, "branch-lookup");
    assert!(result.path.exists());
}

#[test]
fn resolve_strict_unknown_returns_err() {
    let repo = TestRepo::new();
    let result = resolve_target_strict(repo.path(), "nonexistent");
    assert!(result.is_err(), "Expected Err for unknown target, got Ok");
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
