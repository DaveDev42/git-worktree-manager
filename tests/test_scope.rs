//! Integration tests for cwd-based scope discovery.

mod common;
use common::TestRepo;

use git_worktree_manager::scope::discover_scope;

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
