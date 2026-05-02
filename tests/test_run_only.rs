//! Integration tests for `gw run --only <glob>`.
//!
//! Regression: `--only` previously matched only against the worktree
//! directory basename (e.g. `myproj-feat-search`), so the documented
//! `--only 'feat-*'` pattern silently matched zero worktrees. The fix
//! makes `--only` match against branch name OR directory basename, so
//! both the README example and the path-layout form work.

mod common;
use common::TestRepo;

/// `--only 'feat-*'` matches the branch name, not just the dir basename.
#[test]
fn only_matches_branch_glob() {
    let repo = TestRepo::new();
    repo.create_worktree("feat-search");
    repo.create_worktree("fix-auth");

    let stdout = repo.cw_stdout(&["run", "--only", "feat-*", "--", "pwd"]);
    assert!(
        stdout.contains("feat-search"),
        "expected feat-search in output, got: {:?}",
        stdout
    );
    assert!(
        !stdout.contains("fix-auth"),
        "fix-auth should be filtered out, got: {:?}",
        stdout
    );
}

/// Exact branch name as a literal pattern matches.
#[test]
fn only_matches_exact_branch_name() {
    let repo = TestRepo::new();
    repo.create_worktree("feat-search");
    repo.create_worktree("fix-auth");

    let stdout = repo.cw_stdout(&["run", "--only", "fix-auth", "--", "pwd"]);
    assert!(
        stdout.contains("fix-auth"),
        "expected fix-auth in output, got: {:?}",
        stdout
    );
    assert!(
        !stdout.contains("feat-search"),
        "feat-search should be filtered out, got: {:?}",
        stdout
    );
}

/// The path-layout form (`<repo>-<branch>`) still matches — backward compatible.
#[test]
fn only_matches_dir_basename_glob() {
    let repo = TestRepo::new();
    repo.create_worktree("feat-search");

    // The dir basename is "<repo-name>-feat-search". A '*feat*' glob
    // crosses the prefix boundary and matches via the basename path.
    let stdout = repo.cw_stdout(&["run", "--only", "*feat*", "--", "pwd"]);
    assert!(
        stdout.contains("feat-search"),
        "expected feat-search via dir-basename match, got: {:?}",
        stdout
    );
}

/// Non-matching glob filters every worktree out.
#[test]
fn only_no_match_yields_empty_output() {
    let repo = TestRepo::new();
    repo.create_worktree("feat-search");

    let stdout = repo.cw_stdout(&["run", "--only", "nonexistent-*", "--", "pwd"]);
    assert!(
        stdout.trim().is_empty(),
        "expected empty output for non-matching glob, got: {:?}",
        stdout
    );
}
