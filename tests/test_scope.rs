//! Integration tests for cwd-based scope discovery.

mod common;
use common::TestRepo;

use std::path::Path;
use std::process::Command;

use git_worktree_manager::scope::discover_scope;

// ---------------------------------------------------------------------------
// Walk-down test helpers (case 2: cwd outside any repo)
// ---------------------------------------------------------------------------

fn git_walk(path: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(path)
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@test.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@test.com")
        .output()
        .expect("git failed");
    assert!(
        output.status.success(),
        "git {:?} failed: {}{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

fn init_repo_at(path: &Path) {
    std::fs::create_dir_all(path).unwrap();
    git_walk(path, &["init", "-b", "main"]);
    git_walk(path, &["config", "user.name", "Test"]);
    git_walk(path, &["config", "user.email", "test@test.com"]);
    git_walk(path, &["config", "commit.gpgsign", "false"]);
    std::fs::write(path.join("README.md"), "# Test\n").unwrap();
    git_walk(path, &["add", "."]);
    git_walk(path, &["commit", "-m", "Initial commit"]);
}

fn add_worktree_at(repo_path: &Path, branch: &str, dest_path: &Path) {
    git_walk(
        repo_path,
        &["worktree", "add", dest_path.to_str().unwrap(), "-b", branch],
    );
}

#[test]
fn scope_inside_worktree_returns_family() {
    let repo = TestRepo::new();
    let _wt_x = repo.create_worktree("feat-x");
    let _wt_y = repo.create_worktree("feat-y");

    // cwd at feat-x worktree
    let wt_x_path = repo.path().parent().unwrap().join(format!(
        "{}-feat-x",
        repo.path().file_name().unwrap().to_str().unwrap()
    ));

    let scope = discover_scope(&wt_x_path).expect("scope discovery");
    let names: Vec<&str> = scope.worktrees().iter().map(|w| w.name.as_str()).collect();

    // The main repo's basename (TempDir name) is unpredictable, so verify
    // the family by counts and presence of feat-x / feat-y.
    assert_eq!(
        scope.worktrees().len(),
        3,
        "main + feat-x + feat-y; got {names:?}"
    );
    let suffixes: Vec<bool> = scope
        .worktrees()
        .iter()
        .map(|w| w.name.ends_with("-feat-x") || w.name.ends_with("-feat-y") || w.is_main)
        .collect();
    assert!(
        suffixes.iter().all(|b| *b),
        "every member should be main or feat-{{x,y}}: {names:?}"
    );

    let main_count = scope.worktrees().iter().filter(|w| w.is_main).count();
    assert_eq!(main_count, 1, "exactly one main");
}

#[test]
fn scope_outside_walks_down_and_unions_repos() {
    let parent = tempfile::TempDir::new().unwrap();
    let repo_a = parent.path().join("a");
    let repo_b = parent.path().join("b");
    init_repo_at(&repo_a);
    init_repo_at(&repo_b);

    // Place each worktree as a sibling under parent so the walk reaches the
    // main repos (a/ and b/) and collect_family enumerates their worktrees.
    let wt_a = parent.path().join("a-feat-1");
    let wt_b = parent.path().join("b-feat-2");
    add_worktree_at(&repo_a, "feat-1", &wt_a);
    add_worktree_at(&repo_b, "feat-2", &wt_b);

    let scope = discover_scope(parent.path()).expect("scope");
    let branches: Vec<Option<&str>> = scope
        .worktrees()
        .iter()
        .map(|w| w.branch.as_deref())
        .collect();

    assert!(
        branches.contains(&Some("feat-1")),
        "missing feat-1: {branches:?}"
    );
    assert!(
        branches.contains(&Some("feat-2")),
        "missing feat-2: {branches:?}"
    );
    // Each repo contributes 2 entries (main + 1 worktree) → 4 total.
    assert_eq!(
        scope.worktrees().len(),
        4,
        "expected 4 entries, got {branches:?}"
    );
}

#[test]
fn scope_outside_no_repos_errors() {
    let parent = tempfile::TempDir::new().unwrap();
    let result = discover_scope(parent.path());
    assert!(result.is_err(), "expected Err for empty parent dir");
}
