//! Integration tests pinning the `gw clean -i` redirect to `gw delete -i`.

mod common;
use common::TestRepo;

/// `gw clean -i` must exit non-zero and mention `gw delete -i` in its output
/// so users know where the interactive flow moved to.
#[test]
fn clean_interactive_is_removed_and_points_at_delete() {
    let repo = TestRepo::new();
    let output = repo.cw(&["clean", "-i"]);

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(
        !output.status.success(),
        "gw clean -i should exit non-zero after the consolidation; exit code: {:?}\noutput: {}",
        output.status.code(),
        combined
    );

    assert!(
        combined.contains("gw delete -i"),
        "gw clean -i output should mention 'gw delete -i' to redirect the user; got:\n{}",
        combined
    );
}

/// `gw clean --help` must mention `gw delete -i` so users discover the
/// interactive flow even when reading help text.
#[test]
fn clean_help_mentions_delete_i_redirect() {
    let repo = TestRepo::new();
    let output = repo.cw(&["clean", "--help"]);

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();

    assert!(
        output.status.success(),
        "gw clean --help should exit 0; got: {:?}\nstdout: {}\nstderr: {}",
        output.status.code(),
        stdout,
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(
        stdout.contains("gw delete -i"),
        "gw clean --help should mention 'gw delete -i' for interactive use; got:\n{}",
        stdout
    );
}

/// `gw clean` with no filter flags must exit 2 (misuse) and tell the user
/// which filters are valid plus where the interactive flow moved.
#[test]
fn clean_with_no_filters_exits_two() {
    let repo = TestRepo::new();
    let output = repo.cw(&["clean"]);

    assert_eq!(
        output.status.code(),
        Some(2),
        "gw clean with no filters should exit 2; stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("--merged") && combined.contains("--older-than"),
        "error should list both filters; got: {}",
        combined
    );
    assert!(
        combined.contains("gw delete -i"),
        "error should redirect to 'gw delete -i'; got: {}",
        combined
    );
}

/// `gw clean --merged` against a real merge-commit branch must delete the
/// worktree, prune metadata, and exit 0.
#[test]
fn clean_merged_deletes_and_exits_zero() {
    let repo = TestRepo::new();

    let wt_path = repo.create_worktree("feat-merge-exit");
    TestRepo::commit_file_at(&wt_path, "feat.txt", "feature work", "feat: add feature");

    repo.git(&["checkout", "main"]);
    repo.git(&[
        "merge",
        "--no-ff",
        "feat-merge-exit",
        "-m",
        "Merge feat-merge-exit",
    ]);

    let output = repo.cw(&["clean", "--merged"]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "gw clean --merged should exit 0 on success; stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(
        !wt_path.exists(),
        "worktree directory should be gone after clean --merged; still exists at {:?}",
        wt_path
    );

    let worktree_list = repo.git_stdout(&["worktree", "list"]);
    assert!(
        !worktree_list.contains("feat-merge-exit"),
        "git worktree list should no longer mention the cleaned worktree; got:\n{}",
        worktree_list
    );
}

/// `gw clean --merged --dry-run` must exit 0 and leave the worktree intact.
#[test]
fn clean_merged_dry_run_preserves_worktree() {
    let repo = TestRepo::new();

    let wt_path = repo.create_worktree("feat-dry-run");
    TestRepo::commit_file_at(&wt_path, "feat.txt", "work", "feat: dry-run candidate");

    repo.git(&["checkout", "main"]);
    repo.git(&[
        "merge",
        "--no-ff",
        "feat-dry-run",
        "-m",
        "Merge feat-dry-run",
    ]);

    let output = repo.cw(&["clean", "--merged", "--dry-run"]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "gw clean --merged --dry-run should exit 0; stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(
        wt_path.exists(),
        "dry-run must not delete the worktree; missing at {:?}",
        wt_path
    );

    let worktree_list = repo.git_stdout(&["worktree", "list"]);
    assert!(
        worktree_list.contains("feat-dry-run"),
        "dry-run must not prune the worktree from the registry; got:\n{}",
        worktree_list
    );
}
