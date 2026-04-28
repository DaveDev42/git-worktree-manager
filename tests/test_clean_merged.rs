//! Integration-level regression tests for `gw clean --merged`.
//!
//! These tests exercise the end-to-end `clean --merged` CLI path using a real
//! git repository and the `gw` binary.  Merge-predicate unit tests that need
//! PrCache injection live in `src/operations/clean.rs` (unit tests behind
//! `#[cfg(test)]`), because `#[cfg(test)]` env-var hooks in `pr_cache.rs`
//! are only active when the library itself is in test mode — not from external
//! integration-test binaries.

mod common;
use common::TestRepo;

/// When a branch is merged via a real (non-squash) merge commit, `gw clean
/// --merged --dry-run` must list it for deletion.
#[test]
fn clean_merged_lists_real_merge_commit_branch() {
    let repo = TestRepo::new();

    // Create a feature worktree
    let _wt_path = repo.create_worktree("feat-to-merge");

    // Make a commit on the feature branch
    TestRepo::commit_file_at(
        &repo.path().parent().unwrap().join(format!(
            "{}-feat-to-merge",
            repo.path().file_name().unwrap().to_str().unwrap()
        )),
        "feat.txt",
        "feature work",
        "feat: add feature",
    );

    // Merge it into main with a merge commit (not squash)
    repo.git(&["checkout", "main"]);
    repo.git(&[
        "merge",
        "--no-ff",
        "feat-to-merge",
        "-m",
        "Merge feat-to-merge",
    ]);

    // clean --merged --dry-run must now list it
    let output = repo.cw_stdout(&["clean", "--merged", "--dry-run"]);
    assert!(
        output.contains("feat-to-merge"),
        "clean --merged --dry-run should list feat-to-merge after a real merge commit; got: {}",
        output
    );
    assert!(
        output.contains("DRY RUN") || output.contains("Would delete"),
        "output should indicate dry-run mode; got: {}",
        output
    );
}

/// When no worktrees are merged (branch has unique commits not reachable from
/// main), `gw clean --merged --dry-run` must report "No worktrees match".
#[test]
fn clean_merged_reports_none_when_no_merged_branches() {
    let repo = TestRepo::new();
    let wt_path = repo.create_worktree("feat-unmerged");

    // Add a unique commit so the branch diverges from main and is genuinely
    // unmerged (a branch at the exact same SHA as main is trivially "merged").
    TestRepo::commit_file_at(&wt_path, "feat.txt", "work", "feat: work in progress");

    let output = repo.cw_stdout(&["clean", "--merged", "--dry-run"]);
    assert!(
        output.contains("No worktrees match"),
        "should say no worktrees match when nothing is merged; got: {}",
        output
    );
}

/// `gw clean --older-than 0` must list all worktrees (age >= 0 days is always
/// true) — verifies the age path still works independently of --merged.
#[test]
fn clean_older_than_zero_lists_worktrees() {
    let repo = TestRepo::new();
    repo.create_worktree("feat-age-test");

    let output = repo.cw_stdout(&["clean", "--older-than", "0", "--dry-run"]);
    assert!(
        output.contains("feat-age-test"),
        "clean --older-than 0 --dry-run should list the worktree; got: {}",
        output
    );
}
