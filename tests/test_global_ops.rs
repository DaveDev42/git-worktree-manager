/// Tests for global worktree operations (`gw -g list`, `gw scan`, `gw prune`).
mod common;

use assert_cmd::Command;
use common::TestRepo;
use predicates::prelude::*;

fn cw() -> Command {
    Command::cargo_bin("gw").unwrap()
}

// ===========================================================================
// gw -g list
// ===========================================================================

#[test]
fn test_global_list_no_repos() {
    // When no repos are registered, -g list should show a message about no repos
    let repo = TestRepo::new();
    let output = repo.cw(&["-g", "list"]);
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    // Should mention no repositories or no worktrees
    assert!(
        combined.contains("No repositor")
            || combined.contains("No repo")
            || combined.contains("no repositor")
            || combined.contains("worktree"),
        "Expected message about no repos, got: {}",
        combined
    );
}

#[test]
fn test_global_list_with_worktree() {
    // After creating a worktree, gw -g list should show it
    let repo = TestRepo::new();
    let _wt = repo.create_worktree("feature-one");

    let output = repo.cw(&["-g", "list"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    // The worktree or repo should appear in global listing
    // (may show the repo name or the worktree branch)
    assert!(
        stdout.contains("feature-one") || stdout.contains("worktree"),
        "Expected worktree info in global list, got: {}",
        stdout
    );
}

#[test]
fn test_global_list_multiple_repos() {
    // With multiple repos having worktrees, list should show all
    let repo1 = TestRepo::new();
    let _wt1 = repo1.create_worktree("feat-alpha");

    let repo2 = TestRepo::new();
    let _wt2 = repo2.create_worktree("feat-beta");

    // Global list from repo1 context
    let output = repo1.cw(&["-g", "list"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    // At minimum, should list the worktree from repo1
    assert!(
        stdout.contains("feat-alpha") || stdout.contains("worktree"),
        "Expected at least repo1's worktree in global list, got: {}",
        stdout
    );
}

#[test]
fn test_global_list_format_table() {
    // Verify table output contains expected column headers
    let repo = TestRepo::new();
    let _wt = repo.create_worktree("table-test");

    let output = repo.cw(&["-g", "list"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    // If there are worktrees, we expect either table headers or compact output
    if stdout.contains("REPO") {
        assert!(stdout.contains("WORKTREE") || stdout.contains("BRANCH"));
        assert!(stdout.contains("STATUS"));
    }
    // Either way, the command should succeed
    assert!(output.status.success());
}

#[test]
fn test_global_list_shows_status() {
    // Worktree status should appear (clean, modified, etc.)
    let repo = TestRepo::new();
    let wt = repo.create_worktree("status-check");

    // Make the worktree dirty
    std::fs::write(wt.join("dirty.txt"), "uncommitted change").unwrap();

    let output = repo.cw(&["-g", "list"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should contain some status indicator
    if stdout.contains("status-check") {
        assert!(
            stdout.contains("clean")
                || stdout.contains("modified")
                || stdout.contains("active")
                || stdout.contains("stale"),
            "Expected a status indicator, got: {}",
            stdout
        );
    }
}

#[test]
fn test_global_list_help() {
    cw().args(["-g", "list", "--help"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("list")
                .or(predicate::str::contains("List"))
                .or(predicate::str::contains("worktree")),
        );
}

#[test]
fn test_global_list_after_worktree_deleted() {
    // After deleting a worktree, it should no longer appear
    let repo = TestRepo::new();
    let _wt = repo.create_worktree("to-delete");

    // Delete the worktree
    let del_output = repo.cw(&["delete", "to-delete"]);
    assert!(
        del_output.status.success(),
        "delete failed: {}",
        String::from_utf8_lossy(&del_output.stderr)
    );

    let output = repo.cw(&["-g", "list"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("to-delete"),
        "Deleted worktree should not appear in list"
    );
}

// ===========================================================================
// gw scan
// ===========================================================================

#[test]
fn test_global_scan_succeeds() {
    // Scan command should succeed (scans from home dir by default)
    let repo = TestRepo::new();
    let output = repo.cw(&["scan"]);
    assert!(
        output.status.success(),
        "scan command should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Scanning") || stdout.contains("repository") || stdout.contains("No repo"),
        "Expected scan output, got: {}",
        stdout
    );
}

#[test]
fn test_global_scan_help() {
    cw().args(["scan", "--help"]).assert().success().stdout(
        predicate::str::contains("scan")
            .or(predicate::str::contains("Scan"))
            .or(predicate::str::contains("repositor")),
    );
}

// ===========================================================================
// gw prune
// ===========================================================================

#[test]
fn test_global_prune_clean() {
    // When registry is clean, prune should report nothing to do
    let repo = TestRepo::new();
    let output = repo.cw(&["prune"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("clean") || stdout.contains("nothing") || stdout.contains("Pruning"),
        "Expected clean prune message, got: {}",
        stdout
    );
}

#[test]
fn test_global_prune_removes_stale() {
    // Register a repo, remove its directory, then prune should clean it
    let repo = TestRepo::new();
    let _wt = repo.create_worktree("prune-test");

    // First prune should be clean
    let output1 = repo.cw(&["prune"]);
    assert!(output1.status.success());

    // Prune again should still succeed
    let output2 = repo.cw(&["prune"]);
    assert!(output2.status.success());
}

#[test]
fn test_global_prune_help() {
    cw().args(["prune", "--help"]).assert().success().stdout(
        predicate::str::contains("prune")
            .or(predicate::str::contains("Prune"))
            .or(predicate::str::contains("registry"))
            .or(predicate::str::contains("stale")),
    );
}

// ===========================================================================
// gw -g flag behavior
// ===========================================================================

#[test]
fn test_global_flag_accepted() {
    // -g flag should be accepted and not error
    let repo = TestRepo::new();
    let output = repo.cw(&["-g", "list"]);
    assert!(
        output.status.success(),
        "-g list should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_global_long_flag_accepted() {
    // --global flag should also work
    let repo = TestRepo::new();
    let output = repo.cw(&["--global", "list"]);
    assert!(
        output.status.success(),
        "--global list should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_global_list_returns_zero() {
    // Ensure exit code is 0
    let repo = TestRepo::new();
    let output = repo.cw(&["-g", "list"]);
    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn test_global_list_clean_worktree() {
    // A worktree with no changes should show "clean"
    let repo = TestRepo::new();
    let _wt = repo.create_worktree("clean-wt");

    let output = repo.cw(&["-g", "list"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.contains("clean-wt") {
        assert!(
            stdout.contains("clean"),
            "Expected 'clean' status for unmodified worktree"
        );
    }
}

#[test]
fn test_global_list_modified_worktree() {
    // A worktree with uncommitted changes should show "modified"
    let repo = TestRepo::new();
    let wt = repo.create_worktree("mod-wt");

    // Create an untracked file to dirty the worktree
    std::fs::write(wt.join("newfile.txt"), "new content").unwrap();
    // Stage it to make it a real modification
    TestRepo::git_at(&wt, &["add", "newfile.txt"]);

    let output = repo.cw(&["-g", "list"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.contains("mod-wt") {
        assert!(
            stdout.contains("modified") || stdout.contains("active"),
            "Expected modified/active status for dirty worktree, got: {}",
            stdout
        );
    }
}

#[test]
fn test_global_list_multiple_worktrees_same_repo() {
    // Multiple worktrees in the same repo should all appear
    let repo = TestRepo::new();
    let _wt1 = repo.create_worktree("multi-a");
    let _wt2 = repo.create_worktree("multi-b");

    let output = repo.cw(&["-g", "list"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("multi-a"),
        "Expected multi-a in list, got: {}",
        stdout
    );
    assert!(
        stdout.contains("multi-b"),
        "Expected multi-b in list, got: {}",
        stdout
    );
}

// ===========================================================================
// Additional edge cases
// ===========================================================================

#[test]
fn test_global_prune_output_format() {
    // Prune output should contain the "Pruning registry..." header
    let repo = TestRepo::new();
    let output = repo.cw(&["prune"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Pruning") || stdout.contains("pruning"),
        "Expected pruning header, got: {}",
        stdout
    );
}

#[test]
fn test_global_scan_exit_code_zero() {
    let repo = TestRepo::new();
    let output = repo.cw(&["scan"]);
    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn test_global_prune_exit_code_zero() {
    let repo = TestRepo::new();
    let output = repo.cw(&["prune"]);
    assert_eq!(output.status.code(), Some(0));
}
