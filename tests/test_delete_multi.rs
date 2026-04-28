//! Integration tests for multi-target `gw delete`.
//!
//! Uses the `TestRepo` harness from `tests/common/`.

mod common;
use common::TestRepo;

/// Match the exact feature-worktree row prefix in `gw list` output
/// (symbol + one space + branch + trailing space), which avoids
/// false-positives against `main` or any default-branch prefix.
fn worktree_row(branch: &str) -> String {
    format!("\u{25cb} {branch} ")
}

#[test]
fn test_delete_multiple_positional_all_succeed() {
    let repo = TestRepo::new();
    assert!(repo.cw_ok(&["new", "a", "--no-term"]));
    assert!(repo.cw_ok(&["new", "b", "--no-term"]));
    assert!(repo.cw_ok(&["new", "c", "--no-term"]));

    let out = repo.cw(&["delete", "a", "b", "c"]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let list = repo.cw_stdout(&["list"]);
    assert!(
        !list.contains(&worktree_row("a")),
        "list still mentions a: {list}"
    );
    assert!(
        !list.contains(&worktree_row("b")),
        "list still mentions b: {list}"
    );
    assert!(
        !list.contains(&worktree_row("c")),
        "list still mentions c: {list}"
    );
}

#[test]
fn test_delete_multiple_mixed_valid_and_missing() {
    let repo = TestRepo::new();
    assert!(repo.cw_ok(&["new", "real", "--no-term"]));

    let out = repo.cw(&["delete", "real", "does-not-exist"]);
    // exit code 2: at least one target was not deleted
    assert_eq!(
        out.status.code(),
        Some(2),
        "stdout: {} stderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let list = repo.cw_stdout(&["list"]);
    assert!(
        !list.contains(&worktree_row("real")),
        "list still mentions real: {list}"
    );
}

#[test]
fn test_delete_dry_run_does_not_delete() {
    let repo = TestRepo::new();
    let p_path = repo.create_worktree("p");
    let q_path = repo.create_worktree("q");

    // Add unique commits so the branches diverge from main and won't be
    // counted as "merged" by git branch --merged (a branch at the same SHA
    // as main is trivially merged).
    TestRepo::commit_file_at(&p_path, "p.txt", "p work", "feat: p work");
    TestRepo::commit_file_at(&q_path, "q.txt", "q work", "feat: q work");

    let out = repo.cw(&["delete", "p", "q", "--dry-run"]);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("dry-run; nothing deleted"),
        "stdout: {stdout}"
    );

    let list = repo.cw_stdout(&["list"]);
    assert!(
        list.contains(&worktree_row("p")),
        "p should still be listed: {list}"
    );
    assert!(
        list.contains(&worktree_row("q")),
        "q should still be listed: {list}"
    );
}

#[test]
fn test_delete_keep_branch_applies_to_all_targets() {
    let repo = TestRepo::new();
    assert!(repo.cw_ok(&["new", "k1", "--no-term"]));
    assert!(repo.cw_ok(&["new", "k2", "--no-term"]));

    let out = repo.cw(&["delete", "k1", "k2", "--keep-branch"]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let branches = repo.git_stdout(&["branch", "--list"]);
    assert!(branches.contains("k1"), "branches: {branches}");
    assert!(branches.contains("k2"), "branches: {branches}");

    // worktrees themselves should be gone
    let list = repo.cw_stdout(&["list"]);
    assert!(
        !list.contains(&worktree_row("k1")),
        "list still mentions k1: {list}"
    );
    assert!(
        !list.contains(&worktree_row("k2")),
        "list still mentions k2: {list}"
    );
}

#[test]
fn test_delete_interactive_conflicts_with_positional_at_runtime() {
    let repo = TestRepo::new();
    let out = repo.cw(&["delete", "-i", "some-target"]);
    assert!(
        !out.status.success(),
        "stdout: {} stderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("cannot") || err.contains("conflict"),
        "expected conflict error, got: {err}"
    );
}

#[test]
fn test_delete_no_args_still_uses_legacy_path() {
    // Running `gw delete` from *inside* a worktree should still delete the
    // current worktree. This exercises the legacy path.
    let repo = TestRepo::new();
    let wt_path = repo.create_worktree("inside-me");

    // Run `gw delete` with cwd = the worktree path, using cw_at
    let out = TestRepo::cw_at(&wt_path, &["delete"]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let list = repo.cw_stdout(&["list"]);
    assert!(
        !list.contains(&worktree_row("inside-me")),
        "list still mentions inside-me: {list}"
    );
}
