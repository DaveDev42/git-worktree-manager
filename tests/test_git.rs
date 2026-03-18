/// Integration tests for git utilities using real git repos.
mod common;

use common::TestRepo;

#[test]
fn test_get_repo_root() {
    let repo = TestRepo::new();
    let root = claude_worktree::git::get_repo_root(Some(repo.path())).unwrap();
    // Compare file names to avoid Windows UNC path (\\?\) vs git path (C:/) mismatch
    assert_eq!(root.file_name().unwrap(), repo.path().file_name().unwrap());
}

#[test]
fn test_get_current_branch() {
    let repo = TestRepo::new();
    let branch = claude_worktree::git::get_current_branch(Some(repo.path())).unwrap();
    // Default branch could be main or master depending on git config
    assert!(branch == "main" || branch == "master");
}

#[test]
fn test_branch_exists() {
    let repo = TestRepo::new();
    let branch = claude_worktree::git::get_current_branch(Some(repo.path())).unwrap();
    assert!(claude_worktree::git::branch_exists(
        &branch,
        Some(repo.path())
    ));
    assert!(!claude_worktree::git::branch_exists(
        "nonexistent-xyz",
        Some(repo.path())
    ));
}

#[test]
fn test_parse_worktrees() {
    let repo = TestRepo::new();
    let worktrees = claude_worktree::git::parse_worktrees(repo.path()).unwrap();
    assert!(!worktrees.is_empty());
    // First entry should be the main repo
    assert_eq!(
        worktrees[0].1.canonicalize().unwrap(),
        repo.path().canonicalize().unwrap()
    );
}

#[test]
fn test_get_feature_worktrees_empty() {
    let repo = TestRepo::new();
    let features = claude_worktree::git::get_feature_worktrees(Some(repo.path())).unwrap();
    assert!(features.is_empty());
}

#[test]
fn test_is_valid_branch_name() {
    let repo = TestRepo::new();
    assert!(claude_worktree::git::is_valid_branch_name(
        "feature-abc",
        Some(repo.path())
    ));
    assert!(claude_worktree::git::is_valid_branch_name(
        "feat/auth",
        Some(repo.path())
    ));
    assert!(!claude_worktree::git::is_valid_branch_name(
        "",
        Some(repo.path())
    ));
    assert!(!claude_worktree::git::is_valid_branch_name(
        "bad..name",
        Some(repo.path())
    ));
    assert!(!claude_worktree::git::is_valid_branch_name(
        "bad name",
        Some(repo.path())
    ));
}

#[test]
fn test_get_set_config() {
    let repo = TestRepo::new();
    claude_worktree::git::set_config("test.key", "test-value", Some(repo.path())).unwrap();
    let value = claude_worktree::git::get_config("test.key", Some(repo.path()));
    assert_eq!(value, Some("test-value".to_string()));
}

#[test]
fn test_unset_config() {
    let repo = TestRepo::new();
    claude_worktree::git::set_config("test.remove", "value", Some(repo.path())).unwrap();
    claude_worktree::git::unset_config("test.remove", Some(repo.path()));
    let value = claude_worktree::git::get_config("test.remove", Some(repo.path()));
    assert!(value.is_none());
}

#[test]
fn test_normalize_branch_name() {
    assert_eq!(
        claude_worktree::git::normalize_branch_name("refs/heads/main"),
        "main"
    );
    assert_eq!(
        claude_worktree::git::normalize_branch_name("feature"),
        "feature"
    );
}

#[test]
fn test_get_branch_name_error() {
    assert!(claude_worktree::git::get_branch_name_error("").contains("empty"));
    assert!(claude_worktree::git::get_branch_name_error("@").contains("'@'"));
    assert!(claude_worktree::git::get_branch_name_error("foo.lock").contains(".lock"));
    assert!(claude_worktree::git::get_branch_name_error("/foo").contains("start or end"));
    assert!(claude_worktree::git::get_branch_name_error("a//b").contains("consecutive slashes"));
    assert!(claude_worktree::git::get_branch_name_error("a..b").contains("consecutive dots"));
}
