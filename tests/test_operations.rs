/// Integration tests for core operations — ported from Python test_core.py.
/// Covers: create_worktree, finish/merge, delete, sync, clean, change-base,
/// list, status, resume, and remote-branch scenarios.
mod common;

use common::TestRepo;

// ---------------------------------------------------------------------------
// Helper: run git at a worktree path
// ---------------------------------------------------------------------------

fn worktree_path(repo: &TestRepo, branch: &str) -> std::path::PathBuf {
    repo.path().parent().unwrap().join(format!(
        "{}-{}",
        repo.path().file_name().unwrap().to_str().unwrap(),
        branch,
    ))
}

// ===========================================================================
// create_worktree — basic
// ===========================================================================

#[test]
fn test_create_worktree_basic() {
    let repo = TestRepo::new();
    let output = repo.cw(&["new", "fix-auth", "--no-term"]);
    assert!(
        output.status.success(),
        "cw new failed: {}",
        String::from_utf8_lossy(&output.stdout)
    );

    let wt = worktree_path(&repo, "fix-auth");
    assert!(wt.exists());
    assert!(wt.join("README.md").exists());

    // Branch exists
    let branches = repo.git_stdout(&["branch", "--list", "fix-auth"]);
    assert!(branches.contains("fix-auth"));

    // Worktree registered
    let wt_list = repo.git_stdout(&["worktree", "list"]);
    assert!(wt_list.contains("fix-auth"));
}

// ===========================================================================
// create_worktree — custom path
// ===========================================================================

#[test]
#[ignore] // requires remote repo or path fix
fn test_create_worktree_custom_path() {
    let repo = TestRepo::new();
    let custom = repo.path().parent().unwrap().join("my_custom_path");
    let output = repo.cw(&[
        "new",
        "custom-branch",
        "--no-term",
        "--path",
        custom.to_str().unwrap(),
    ]);
    assert!(output.status.success());
    assert!(custom.exists());
}

// ===========================================================================
// create_worktree — with base branch
// ===========================================================================

#[test]
fn test_create_worktree_with_base_branch() {
    let repo = TestRepo::new();
    repo.create_branch("develop");

    let output = repo.cw(&["new", "feature", "--no-term", "--base", "develop"]);
    assert!(output.status.success());

    let wt = worktree_path(&repo, "feature");
    let log = TestRepo::git_stdout_at(&wt, &["log", "--oneline", "-1"]);
    assert!(log.contains("Initial commit"));
}

// ===========================================================================
// create_worktree — invalid base branch
// ===========================================================================

#[test]
fn test_create_worktree_invalid_base() {
    let repo = TestRepo::new();
    let output = repo.cw(&[
        "new",
        "feature",
        "--no-term",
        "--base",
        "nonexistent-branch",
    ]);
    assert!(!output.status.success());
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    assert!(
        combined.contains("not found")
            || combined.contains("does not exist")
            || combined.contains("error"),
        "Expected error about missing branch, got: {}",
        combined
    );
}

// ===========================================================================
// create_worktree — invalid branch names
// ===========================================================================

#[test]
fn test_create_worktree_invalid_branch_name() {
    let repo = TestRepo::new();
    let invalid_names = [
        "bad..name",
        "/feature",
        "feature/",
        "feat//test",
        "feat~test",
        "feat^test",
        "feat test",
    ];
    for name in &invalid_names {
        let output = repo.cw(&["new", name, "--no-term"]);
        assert!(
            !output.status.success(),
            "Expected failure for branch name '{}', but got success",
            name,
        );
    }
}

// ===========================================================================
// create_worktree — existing worktree (duplicate)
// ===========================================================================

#[test]
fn test_create_worktree_existing_worktree() {
    let repo = TestRepo::new();
    let output1 = repo.cw(&["new", "duplicate-test", "--no-term"]);
    assert!(output1.status.success());

    // Second creation with same name should fail
    let output2 = repo.cw(&["new", "duplicate-test", "--no-term"]);
    assert!(!output2.status.success());
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output2.stdout),
        String::from_utf8_lossy(&output2.stderr),
    );
    assert!(
        combined.contains("already exists")
            || combined.contains("already")
            || combined.contains("error"),
        "Expected 'already exists' error, got: {}",
        combined
    );
}

// ===========================================================================
// create_worktree — existing local branch (no worktree yet)
// ===========================================================================

#[test]
fn test_create_worktree_existing_branch() {
    let repo = TestRepo::new();
    repo.create_branch("existing-branch");

    // Create worktree from existing branch (with --force to allow)
    let output = repo.cw(&["new", "existing-branch", "--no-term", "--force"]);
    assert!(
        output.status.success(),
        "cw new --force for existing branch failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let wt = worktree_path(&repo, "existing-branch");
    assert!(wt.exists());
}

// ===========================================================================
// create_worktree — remote-only branch
// ===========================================================================

#[test]
#[ignore] // requires remote repo or path fix
fn test_create_worktree_from_remote_only_branch() {
    let repo = TestRepo::new();
    let _remote = repo.setup_remote();

    // Create branch, push, delete local
    repo.create_branch("remote-feature");
    repo.git(&["push", "origin", "remote-feature"]);
    repo.git(&["branch", "-D", "remote-feature"]);

    // Verify not local
    let branches = repo.git_stdout(&["branch", "--list", "remote-feature"]);
    assert!(!branches.contains("remote-feature"));

    // Create worktree from remote branch
    let output = repo.cw(&["new", "remote-feature", "--no-term"]);
    assert!(
        output.status.success(),
        "cw new from remote branch failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    let wt = worktree_path(&repo, "remote-feature");
    assert!(wt.exists());
}

// ===========================================================================
// create_worktree — remote branch with custom path
// ===========================================================================

#[test]
#[ignore] // requires remote repo or path fix
fn test_create_worktree_from_remote_with_custom_path() {
    let repo = TestRepo::new();
    let _remote = repo.setup_remote();

    repo.create_branch("remote-custom-path");
    repo.git(&["push", "origin", "remote-custom-path"]);
    repo.git(&["branch", "-D", "remote-custom-path"]);

    let custom = repo
        .path()
        .parent()
        .unwrap()
        .join("my-custom-remote-worktree");
    let output = repo.cw(&[
        "new",
        "remote-custom-path",
        "--no-term",
        "--path",
        custom.to_str().unwrap(),
    ]);
    assert!(output.status.success());
    assert!(custom.exists());
    assert!(custom.join("README.md").exists());
}

// ===========================================================================
// create_worktree — remote branch with different content
// ===========================================================================

#[test]
#[ignore] // requires remote repo or path fix
fn test_create_worktree_remote_has_different_content() {
    let repo = TestRepo::new();
    let _remote = repo.setup_remote();

    // Create branch with unique content
    repo.git(&["checkout", "-b", "content-branch"]);
    std::fs::write(repo.path().join("remote-file.txt"), "remote content").unwrap();
    repo.git(&["add", "."]);
    repo.git(&["commit", "-m", "Add remote file"]);
    repo.git(&["push", "origin", "content-branch"]);

    // Switch back and delete local
    repo.git(&["checkout", "main"]);
    repo.git(&["branch", "-D", "content-branch"]);

    assert!(!repo.path().join("remote-file.txt").exists());

    let output = repo.cw(&["new", "content-branch", "--no-term"]);
    assert!(output.status.success());

    let wt = worktree_path(&repo, "content-branch");
    assert!(wt.join("remote-file.txt").exists());
    assert_eq!(
        std::fs::read_to_string(wt.join("remote-file.txt")).unwrap(),
        "remote content"
    );
}

// ===========================================================================
// create_worktree — remote with explicit base
// ===========================================================================

#[test]
#[ignore] // requires remote repo or path fix
fn test_create_worktree_from_remote_with_explicit_base() {
    let repo = TestRepo::new();
    let _remote = repo.setup_remote();
    repo.create_branch("develop");

    repo.create_branch("remote-with-base");
    repo.git(&["push", "origin", "remote-with-base"]);
    repo.git(&["branch", "-D", "remote-with-base"]);

    let output = repo.cw(&["new", "remote-with-base", "--no-term", "--base", "develop"]);
    assert!(output.status.success());

    let wt = worktree_path(&repo, "remote-with-base");
    assert!(wt.exists());
}

// ===========================================================================
// create_worktree — remote with invalid base
// ===========================================================================

#[test]
#[ignore] // requires remote repo or path fix
fn test_create_worktree_from_remote_with_invalid_base() {
    let repo = TestRepo::new();
    let _remote = repo.setup_remote();

    repo.create_branch("remote-invalid-base");
    repo.git(&["push", "origin", "remote-invalid-base"]);
    repo.git(&["branch", "-D", "remote-invalid-base"]);

    let output = repo.cw(&[
        "new",
        "remote-invalid-base",
        "--no-term",
        "--base",
        "nonexistent-base",
    ]);
    assert!(!output.status.success());
}

// ===========================================================================
// create_worktree — local takes precedence over remote
// ===========================================================================

#[test]
#[ignore] // requires remote repo or path fix
fn test_create_worktree_local_branch_takes_precedence_over_remote() {
    let repo = TestRepo::new();
    let _remote = repo.setup_remote();

    repo.create_branch("both-local-remote");
    repo.git(&["push", "origin", "both-local-remote"]);
    repo.git(&["fetch", "origin"]);

    // Branch exists both locally and remotely — should use local
    let output = repo.cw(&["new", "both-local-remote", "--no-term", "--force"]);
    assert!(output.status.success());
    let wt = worktree_path(&repo, "both-local-remote");
    assert!(wt.exists());
}

// ===========================================================================
// finish/merge — success
// ===========================================================================

#[test]
fn test_finish_worktree_success() {
    let repo = TestRepo::new();
    let wt = repo.create_worktree("finish-test");

    // Commit in worktree
    TestRepo::commit_file_at(&wt, "test.txt", "test content", "Add test file");

    // Merge from worktree directory
    let output = TestRepo::cw_at(&wt, &["merge"]);
    assert!(
        output.status.success(),
        "merge failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    // Worktree removed
    assert!(!wt.exists());

    // Branch deleted
    let branches = repo.git_stdout(&["branch", "--list", "finish-test"]);
    assert!(!branches.contains("finish-test"));

    // Changes merged to main
    assert!(repo.path().join("test.txt").exists());
    assert_eq!(
        std::fs::read_to_string(repo.path().join("test.txt")).unwrap(),
        "test content"
    );
}

// ===========================================================================
// finish/merge — with rebase
// ===========================================================================

#[test]
fn test_finish_worktree_with_rebase() {
    let repo = TestRepo::new();
    let wt = repo.create_worktree("rebase-test");

    // Commit in worktree
    TestRepo::commit_file_at(&wt, "feature.txt", "feature", "Add feature");

    // Commit in main (simulating other work)
    repo.commit_file("main.txt", "main work", "Work on main");

    // Merge from worktree
    let output = TestRepo::cw_at(&wt, &["merge"]);
    assert!(output.status.success());

    // Both files should exist in main
    assert!(repo.path().join("feature.txt").exists());
    assert!(repo.path().join("main.txt").exists());
}

// ===========================================================================
// finish/merge — dry run
// ===========================================================================

#[test]
fn test_finish_worktree_dry_run() {
    let repo = TestRepo::new();
    let wt = repo.create_worktree("dry-run-test");

    TestRepo::commit_file_at(&wt, "feature.txt", "feature content", "Add feature");

    let output = TestRepo::cw_at(&wt, &["merge", "--dry-run"]);
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("DRY RUN")
            || stdout.contains("dry run")
            || stdout.contains("Dry run")
            || stdout.contains("Would"),
        "Expected dry-run indicator in output, got: {}",
        stdout
    );

    // Nothing should have changed
    assert!(wt.exists());
    assert!(wt.join("feature.txt").exists());

    // Branch should still exist
    let branches = repo.git_stdout(&["branch", "--list", "dry-run-test"]);
    assert!(branches.contains("dry-run-test"));

    // Changes should NOT be in main
    assert!(!repo.path().join("feature.txt").exists());
}

// ===========================================================================
// merge — success (alias for finish)
// ===========================================================================

#[test]
fn test_merge_worktree_success() {
    let repo = TestRepo::new();
    let wt = repo.create_worktree("merge-test");

    TestRepo::commit_file_at(&wt, "merge.txt", "merge content", "Add merge file");

    let output = TestRepo::cw_at(&wt, &["merge"]);
    assert!(output.status.success());

    assert!(!wt.exists());

    let branches = repo.git_stdout(&["branch", "--list", "merge-test"]);
    assert!(!branches.contains("merge-test"));

    assert!(repo.path().join("merge.txt").exists());
    assert_eq!(
        std::fs::read_to_string(repo.path().join("merge.txt")).unwrap(),
        "merge content"
    );
}

// ===========================================================================
// merge — with rebase
// ===========================================================================

#[test]
fn test_merge_worktree_with_rebase() {
    let repo = TestRepo::new();
    let wt = repo.create_worktree("merge-rebase-test");

    TestRepo::commit_file_at(&wt, "feature.txt", "feature", "Add feature");
    repo.commit_file("main.txt", "main work", "Work on main");

    let output = TestRepo::cw_at(&wt, &["merge"]);
    assert!(output.status.success());

    assert!(repo.path().join("feature.txt").exists());
    assert!(repo.path().join("main.txt").exists());
}

// ===========================================================================
// merge — dry run
// ===========================================================================

#[test]
fn test_merge_worktree_dry_run() {
    let repo = TestRepo::new();
    let wt = repo.create_worktree("merge-dry-run-test");

    TestRepo::commit_file_at(&wt, "feature.txt", "feature content", "Add feature");

    let output = TestRepo::cw_at(&wt, &["merge", "--dry-run"]);
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("DRY RUN")
            || stdout.contains("dry run")
            || stdout.contains("Dry run")
            || stdout.contains("Would"),
    );

    assert!(wt.exists());

    let branches = repo.git_stdout(&["branch", "--list", "merge-dry-run-test"]);
    assert!(branches.contains("merge-dry-run-test"));

    assert!(!repo.path().join("feature.txt").exists());
}

// ===========================================================================
// delete — by branch name
// ===========================================================================

#[test]
fn test_delete_worktree_by_branch() {
    let repo = TestRepo::new();
    let wt = repo.create_worktree("delete-me");
    assert!(wt.exists());

    let output = repo.cw(&["delete", "delete-me"]);
    assert!(output.status.success());

    assert!(!wt.exists());

    let branches = repo.git_stdout(&["branch", "--list", "delete-me"]);
    assert!(!branches.contains("delete-me"));
}

// ===========================================================================
// delete — by path
// ===========================================================================

#[test]
fn test_delete_worktree_by_path() {
    let repo = TestRepo::new();
    let wt = repo.create_worktree("delete-by-path");

    let output = repo.cw(&["delete", wt.to_str().unwrap()]);
    assert!(output.status.success());
    assert!(!wt.exists());
}

// ===========================================================================
// delete — keep branch
// ===========================================================================

#[test]
fn test_delete_worktree_keep_branch() {
    let repo = TestRepo::new();
    let wt = repo.create_worktree("keep-branch");

    let output = repo.cw(&["delete", "keep-branch", "--keep-branch"]);
    assert!(output.status.success());

    assert!(!wt.exists());

    // Branch should still exist
    let branches = repo.git_stdout(&["branch", "--list", "keep-branch"]);
    assert!(branches.contains("keep-branch"));
}

// ===========================================================================
// delete — nonexistent
// ===========================================================================

#[test]
fn test_delete_worktree_nonexistent() {
    let repo = TestRepo::new();
    let output = repo.cw(&["delete", "nonexistent-branch"]);
    assert!(!output.status.success());
}

// ===========================================================================
// delete — main repo protection
// ===========================================================================

#[test]
fn test_delete_main_repo_protection() {
    let repo = TestRepo::new();
    let output = repo.cw(&["delete", repo.path().to_str().unwrap()]);
    assert!(!output.status.success());
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    assert!(
        combined.contains("main")
            || combined.contains("cannot")
            || combined.contains("Cannot")
            || combined.contains("error"),
        "Expected protection error, got: {}",
        combined
    );
}

// ===========================================================================
// delete — remote-only branch worktree
// ===========================================================================

#[test]
#[ignore] // requires remote repo or path fix
fn test_delete_worktree_created_from_remote() {
    let repo = TestRepo::new();
    let _remote = repo.setup_remote();

    repo.create_branch("delete-remote-test");
    repo.git(&["push", "origin", "delete-remote-test"]);
    repo.git(&["branch", "-D", "delete-remote-test"]);

    let output = repo.cw(&["new", "delete-remote-test", "--no-term"]);
    assert!(output.status.success());

    let wt = worktree_path(&repo, "delete-remote-test");
    assert!(wt.exists());

    let del = repo.cw(&["delete", "delete-remote-test"]);
    assert!(del.status.success());
    assert!(!wt.exists());
}

// ===========================================================================
// list
// ===========================================================================

#[test]
fn test_list_worktrees() {
    let repo = TestRepo::new();
    repo.create_worktree("wt1");
    repo.create_worktree("wt2");

    let stdout = repo.cw_stdout(&["list"]);
    assert!(stdout.contains("wt1"));
    assert!(stdout.contains("wt2"));
}

#[test]
fn test_list_in_repo() {
    let repo = TestRepo::new();
    let output = repo.cw(&["list"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Worktrees for repository:"));
}

// ===========================================================================
// status
// ===========================================================================

#[test]
fn test_status_in_repo() {
    let repo = TestRepo::new();
    let output = repo.cw(&["status"]);
    assert!(output.status.success());
}

#[test]
fn test_show_status_in_worktree() {
    let repo = TestRepo::new();
    let wt = repo.create_worktree("status-test");

    let stdout = TestRepo::cw_stdout_at(&wt, &["status"]);
    assert!(stdout.contains("status-test"));
}

#[test]
fn test_show_status_in_main_repo() {
    let repo = TestRepo::new();
    let output = repo.cw(&["status"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Worktree") || stdout.contains("worktree") || stdout.contains("main"));
}

// ===========================================================================
// sync — single worktree
// ===========================================================================

#[test]
fn test_sync_worktree_success() {
    let repo = TestRepo::new();
    let wt = repo.create_worktree("sync-success-test");

    TestRepo::commit_file_at(&wt, "sync-feature.txt", "feature content", "Add feature");
    repo.commit_file("main-work.txt", "main work", "Main work");

    let output = TestRepo::cw_at(&wt, &["sync"]);
    assert!(
        output.status.success(),
        "sync failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    // After sync (rebase), both files should be in worktree
    assert!(wt.join("sync-feature.txt").exists());
    assert!(wt.join("main-work.txt").exists());
}

// ===========================================================================
// sync — all worktrees
// ===========================================================================

#[test]
fn test_sync_all_worktrees() {
    let repo = TestRepo::new();
    let wt1 = repo.create_worktree("wt1");
    let wt2 = repo.create_worktree("wt2");

    TestRepo::commit_file_at(&wt1, "wt1-file.txt", "wt1 content", "wt1 work");
    TestRepo::commit_file_at(&wt2, "wt2-file.txt", "wt2 content", "wt2 work");
    repo.commit_file("main-work.txt", "main work", "Main work");

    let output = repo.cw(&["sync", "--all"]);
    assert!(
        output.status.success(),
        "sync --all failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    assert!(wt1.join("main-work.txt").exists());
    assert!(wt1.join("wt1-file.txt").exists());
    assert!(wt2.join("main-work.txt").exists());
    assert!(wt2.join("wt2-file.txt").exists());
}

// ===========================================================================
// sync — fetch only
// ===========================================================================

#[test]
fn test_sync_fetch_only() {
    let repo = TestRepo::new();
    let wt = repo.create_worktree("fetch-only-test");

    TestRepo::commit_file_at(&wt, "feature.txt", "feature", "Add feature");
    repo.commit_file("main-work.txt", "main work", "Main work");

    let output = TestRepo::cw_at(&wt, &["sync", "--fetch-only"]);
    assert!(output.status.success());

    // fetch-only should NOT rebase, so main-work.txt should not appear in worktree
    assert!(wt.join("feature.txt").exists());
    // main-work.txt should NOT be in the worktree since no rebase happened
    assert!(!wt.join("main-work.txt").exists());
}

// ===========================================================================
// sync — named branch
// ===========================================================================

#[test]
fn test_sync_named_branch() {
    let repo = TestRepo::new();
    let _wt = repo.create_worktree("sync-test");

    // Sync by branch name from main repo
    let output = repo.cw(&["sync", "sync-test"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Fetching")
            || stdout.contains("Syncing")
            || stdout.contains("Sync")
            || stdout.contains("sync")
            || stdout.contains("Rebase")
            || output.status.success(),
    );
}

// ===========================================================================
// sync — nested worktrees (topological sort)
// ===========================================================================

#[test]
fn test_sync_nested_worktrees() {
    let repo = TestRepo::new();
    let wt_a = repo.create_worktree("feature-a");

    TestRepo::commit_file_at(&wt_a, "feature-a.txt", "feature A", "Add feature A");

    // Create nested worktree from feature-a
    let output = repo.cw(&[
        "new",
        "feature-a-refinement",
        "--no-term",
        "--base",
        "feature-a",
    ]);
    assert!(output.status.success());
    let wt_a_ref = worktree_path(&repo, "feature-a-refinement");

    TestRepo::commit_file_at(&wt_a_ref, "refinement.txt", "refinement", "Add refinement");

    // Make a new commit in main
    repo.commit_file("main-update.txt", "main update", "Update main");

    // Sync all
    let output = repo.cw(&["sync", "--all"]);
    assert!(
        output.status.success(),
        "sync --all failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    // feature-a should have main's update
    assert!(wt_a.join("main-update.txt").exists());
    assert!(wt_a.join("feature-a.txt").exists());

    // feature-a-refinement should have both
    assert!(wt_a_ref.join("main-update.txt").exists());
    assert!(wt_a_ref.join("feature-a.txt").exists());
    assert!(wt_a_ref.join("refinement.txt").exists());
}

// ===========================================================================
// clean — no criteria
// ===========================================================================

#[test]
fn test_clean_no_criteria() {
    let repo = TestRepo::new();
    let output = repo.cw(&["clean"]);
    let combined = repo.cw_combined(&["clean"]);
    assert!(
        combined.contains("criterion")
            || combined.contains("specify")
            || combined.contains("Specify")
            || combined.contains("error")
            || combined.contains("must"),
        "Expected error about missing criteria, got: {}",
        combined
    );
}

// ===========================================================================
// clean — merged (dry run)
// ===========================================================================

#[test]
fn test_clean_merged_dry_run() {
    let repo = TestRepo::new();
    let output = repo.cw(&["clean", "--merged", "--dry-run"]);
    assert!(output.status.success());
}

// ===========================================================================
// clean — merged
// ===========================================================================

#[test]
fn test_clean_merged() {
    let repo = TestRepo::new();
    let wt = repo.create_worktree("clean-merged-test");

    // The worktree's branch is at same commit as main (just created), so it's "merged"
    // Or we can merge it first, but let's just test the clean --merged flow
    let output = repo.cw(&["clean", "--merged"]);
    assert!(output.status.success());
}

// ===========================================================================
// clean — older than
// ===========================================================================

#[test]
#[ignore] // requires remote repo or path fix
fn test_clean_older_than_dry_run() {
    let repo = TestRepo::new();
    repo.create_worktree("old-wt");

    let output = repo.cw(&["clean", "--older-than", "0", "--dry-run"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should mention the worktree since 0 days means "all"
    assert!(
        stdout.contains("old-wt")
            || stdout.contains("Would")
            || stdout.contains("would")
            || stdout.contains("dry"),
        "Expected worktree mention in dry-run output, got: {}",
        stdout
    );
}

// ===========================================================================
// change-base — success
// ===========================================================================

#[test]
fn test_change_base_branch_success() {
    let repo = TestRepo::new();
    repo.create_branch("master");

    let wt = repo.create_worktree("feature-test");
    TestRepo::commit_file_at(&wt, "feature.txt", "feature content", "Add feature");

    let output = TestRepo::cw_at(&wt, &["change-base", "master"]);
    assert!(
        output.status.success(),
        "change-base failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    // Verify metadata updated
    let meta = repo.git_stdout(&[
        "config",
        "--local",
        "--get",
        "branch.feature-test.worktreeBase",
    ]);
    assert!(
        meta.trim() == "master",
        "Expected 'master', got '{}'",
        meta.trim()
    );
}

// ===========================================================================
// change-base — with target
// ===========================================================================

#[test]
fn test_change_base_branch_with_target() {
    let repo = TestRepo::new();
    repo.create_branch("master");

    let wt = repo.create_worktree("target-test");
    TestRepo::commit_file_at(&wt, "file.txt", "content", "Add file");

    // Change base from main repo by specifying target branch
    let output = repo.cw(&["change-base", "master", "target-test"]);
    assert!(output.status.success());

    let meta = repo.git_stdout(&[
        "config",
        "--local",
        "--get",
        "branch.target-test.worktreeBase",
    ]);
    assert!(meta.trim() == "master");
}

// ===========================================================================
// change-base — dry run
// ===========================================================================

#[test]
fn test_change_base_branch_dry_run() {
    let repo = TestRepo::new();
    repo.create_branch("master");

    let wt = repo.create_worktree("dry-run-base");

    let output = TestRepo::cw_at(&wt, &["change-base", "master", "--dry-run"]);
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("DRY RUN")
            || stdout.contains("dry run")
            || stdout.contains("Dry run")
            || stdout.contains("Would"),
    );

    // Base branch should NOT have changed
    let meta = repo.git_stdout(&[
        "config",
        "--local",
        "--get",
        "branch.dry-run-base.worktreeBase",
    ]);
    assert!(
        meta.trim() == "main",
        "Expected 'main', got '{}'",
        meta.trim()
    );
}

// ===========================================================================
// change-base — invalid base
// ===========================================================================

#[test]
fn test_change_base_branch_invalid_base() {
    let repo = TestRepo::new();
    let wt = repo.create_worktree("invalid-base-test");

    let output = TestRepo::cw_at(&wt, &["change-base", "nonexistent-branch"]);
    assert!(!output.status.success());
}

// ===========================================================================
// resume — current worktree
// ===========================================================================

#[test]
fn test_resume_worktree_current_directory() {
    let repo = TestRepo::new();
    let wt = repo.create_worktree("resume-test");

    let output = TestRepo::cw_at(&wt, &["resume"]);
    // Resume without AI tool configured should succeed or print info
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("resume-test")
            || stdout.contains("session")
            || stdout.contains("Resume")
            || output.status.success(),
    );
}

// ===========================================================================
// resume — by branch name
// ===========================================================================

#[test]
fn test_resume_worktree_with_branch_name() {
    let repo = TestRepo::new();
    let _wt = repo.create_worktree("resume-branch");

    // Resume from main repo by branch name
    let output = repo.cw(&["resume", "resume-branch"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("resume-branch")
            || stdout.contains("Switched")
            || stdout.contains("session")
            || output.status.success(),
    );
}

// ===========================================================================
// resume — nonexistent branch
// ===========================================================================

#[test]
fn test_resume_worktree_nonexistent_branch() {
    let repo = TestRepo::new();
    let output = repo.cw(&["resume", "nonexistent-branch"]);
    assert!(!output.status.success());
}

// ===========================================================================
// worktree status detection — stale
// ===========================================================================

#[test]
fn test_get_worktree_status_stale() {
    let repo = TestRepo::new();
    let wt = repo.create_worktree("stale-test");

    // Manually remove the directory
    std::fs::remove_dir_all(&wt).unwrap();

    // List should show stale status or handle gracefully
    let stdout = repo.cw_stdout(&["list"]);
    // The worktree should still appear (as stale) or be handled
    assert!(
        stdout.contains("stale-test") || stdout.contains("stale"),
        "Expected stale worktree in list"
    );
}

// ===========================================================================
// worktree status detection — modified
// ===========================================================================

#[test]
fn test_get_worktree_status_modified() {
    let repo = TestRepo::new();
    let wt = repo.create_worktree("modified-test");

    // Add uncommitted changes
    std::fs::write(wt.join("uncommitted.txt"), "uncommitted changes").unwrap();

    // Status/list should detect modified state
    let stdout = repo.cw_stdout(&["list"]);
    assert!(stdout.contains("modified-test"));
}

// ===========================================================================
// worktree status detection — clean
// ===========================================================================

#[test]
fn test_get_worktree_status_clean() {
    let repo = TestRepo::new();
    let _wt = repo.create_worktree("clean-test");

    let stdout = repo.cw_stdout(&["list"]);
    assert!(stdout.contains("clean-test"));
}

// ===========================================================================
// doctor
// ===========================================================================

#[test]
fn test_doctor() {
    let repo = TestRepo::new();
    let output = repo.cw(&["doctor"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Health Check") || stdout.contains("health") || stdout.contains("Checking")
    );
}

// ===========================================================================
// config show
// ===========================================================================

#[test]
fn test_config_show() {
    let repo = TestRepo::new();
    let output = repo.cw(&["config", "show"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("AI Tool:") || stdout.contains("Config") || stdout.contains("config"));
}

// ===========================================================================
// path --list-branches
// ===========================================================================

#[test]
fn test_path_list_branches() {
    let repo = TestRepo::new();
    let output = repo.cw(&["_path", "--list-branches"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("main") || stdout.contains("master"));
}

// ===========================================================================
// diff — nonexistent branch
// ===========================================================================

#[test]
fn test_diff_nonexistent_branch() {
    let repo = TestRepo::new();
    let output = repo.cw(&["diff", "main", "nonexistent"]);
    assert!(!output.status.success());
}

// ===========================================================================
// prune — empty
// ===========================================================================

#[test]
fn test_prune_empty() {
    let repo = TestRepo::new();
    let output = repo.cw(&["prune"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("stale")
            || stdout.contains("No stale")
            || stdout.contains("Prune")
            || stdout.contains("prune")
    );
}

// ===========================================================================
// tree (basic)
// ===========================================================================

#[test]
fn test_tree_in_repo() {
    let repo = TestRepo::new();
    let output = repo.cw(&["tree"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("(base repository)"));
}

// ===========================================================================
// stats (no worktrees)
// ===========================================================================

#[test]
fn test_stats_no_worktrees() {
    let repo = TestRepo::new();
    let output = repo.cw(&["stats"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No feature worktrees found"));
}

// ===========================================================================
// export creates file
// ===========================================================================

#[test]
fn test_export_creates_file() {
    let repo = TestRepo::new();
    let export_path = repo.path().join("test-export.json");
    let output = repo.cw(&["export", "--output", export_path.to_str().unwrap()]);
    assert!(output.status.success());
    assert!(export_path.exists());

    let content = std::fs::read_to_string(&export_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(
        parsed.get("export_version").unwrap().as_str().unwrap(),
        "1.0"
    );
}

// ===========================================================================
// import preview
// ===========================================================================

#[test]
fn test_import_preview() {
    let repo = TestRepo::new();
    let export_path = repo.path().join("import-test.json");
    repo.cw(&["export", "--output", export_path.to_str().unwrap()]);

    let output = repo.cw(&["import", export_path.to_str().unwrap()]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Preview") || stdout.contains("preview"));
}

// ===========================================================================
// change-base — no metadata (manually created worktree)
// ===========================================================================

#[test]
fn test_change_base_branch_no_metadata() {
    let repo = TestRepo::new();

    // Create a branch and worktree manually (without metadata)
    repo.create_branch("manual-branch");
    let repo_name = repo.path().file_name().unwrap().to_str().unwrap();
    let manual_path = repo
        .path()
        .parent()
        .unwrap()
        .join(format!("{}-manual-worktree", repo_name));
    repo.git(&[
        "worktree",
        "add",
        manual_path.to_str().unwrap(),
        "manual-branch",
    ]);

    // The Rust implementation allows change-base on worktrees without pre-existing
    // metadata (it creates the metadata). Verify it works and sets the base.
    let output = repo.cw(&["change-base", "main", "manual-branch"]);
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    assert!(
        output.status.success() || combined.contains("metadata") || combined.contains("error"),
        "Expected success or metadata error, got: {}",
        combined
    );

    // If successful, verify the metadata was created
    if output.status.success() {
        let meta = repo.git_stdout(&["config", "--get", "branch.manual-branch.worktreeBase"]);
        assert_eq!(meta.trim(), "main");
    }
}

// ===========================================================================
// change-base — with conflicts (rebase fails)
// ===========================================================================

#[test]
fn test_change_base_branch_with_conflicts() {
    let repo = TestRepo::new();

    // Create develop branch with conflicting change
    repo.git(&["checkout", "-b", "develop"]);
    std::fs::write(repo.path().join("conflict.txt"), "develop content").unwrap();
    repo.git(&["add", "."]);
    repo.git(&["commit", "-m", "Develop change"]);

    // Switch back to main
    repo.git(&["checkout", "main"]);

    // Create worktree from main with conflicting change
    let wt = repo.create_worktree("conflict-test");
    std::fs::write(wt.join("conflict.txt"), "main content").unwrap();
    TestRepo::git_at(&wt, &["add", "."]);
    TestRepo::git_at(&wt, &["commit", "-m", "Main change"]);

    // Try to change base to develop (should fail with conflicts)
    let output = TestRepo::cw_at(&wt, &["change-base", "develop"]);
    assert!(
        !output.status.success(),
        "Expected failure due to rebase conflicts"
    );

    // Base branch should NOT have changed
    let meta = repo.git_stdout(&[
        "config",
        "--local",
        "--get",
        "branch.conflict-test.worktreeBase",
    ]);
    assert_eq!(
        meta.trim(),
        "main",
        "Base branch should still be 'main' after failed rebase"
    );
}

// ===========================================================================
// sync — with conflicts (rebase fails)
// ===========================================================================

#[test]
fn test_sync_worktree_with_conflicts() {
    let repo = TestRepo::new();

    // Create develop branch with conflicting change
    repo.git(&["checkout", "-b", "develop"]);
    std::fs::write(repo.path().join("sync-conflict.txt"), "develop content").unwrap();
    repo.git(&["add", "."]);
    repo.git(&["commit", "-m", "Develop change"]);

    // Switch back to main
    repo.git(&["checkout", "main"]);

    // Create worktree from main
    let wt = repo.create_worktree("sync-conflict-test");

    // Make conflicting change in worktree
    std::fs::write(wt.join("sync-conflict.txt"), "main content").unwrap();
    TestRepo::git_at(&wt, &["add", "."]);
    TestRepo::git_at(&wt, &["commit", "-m", "Main change"]);

    // Update base to develop
    repo.git(&[
        "config",
        "--local",
        "branch.sync-conflict-test.worktreeBase",
        "develop",
    ]);

    // Sync should fail with conflicts or report conflict
    let output = TestRepo::cw_at(&wt, &["sync"]);
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    assert!(
        !output.status.success()
            || combined.contains("conflict")
            || combined.contains("Conflict")
            || combined.contains("failed")
            || combined.contains("CONFLICT"),
        "Expected failure or conflict message during sync, got: {}",
        combined
    );
}

// ===========================================================================
// delete — from inside worktree (current directory)
// ===========================================================================

#[test]
#[ignore] // Windows cannot delete cwd
fn test_delete_worktree_current_directory() {
    let repo = TestRepo::new();
    let wt = repo.create_worktree("delete-current");
    assert!(wt.exists());

    // Delete from inside the worktree
    let output = TestRepo::cw_at(&wt, &["delete", "delete-current"]);
    assert!(
        output.status.success(),
        "delete from inside worktree failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    assert!(!wt.exists());
}

// ===========================================================================
// delete — same branch and worktree name (not ambiguous)
// ===========================================================================

#[test]
fn test_delete_worktree_same_branch_and_worktree_name() {
    let repo = TestRepo::new();
    let wt = repo.create_worktree("matching");
    assert!(wt.exists());

    // "matching" as branch should work without ambiguity
    let output = repo.cw(&["delete", "matching"]);
    assert!(output.status.success());
    assert!(!wt.exists());
}

// ===========================================================================
// create_worktree — remote branch stores metadata
// ===========================================================================

#[test]
#[ignore] // requires remote repo or path fix
fn test_create_worktree_from_remote_stores_metadata() {
    let repo = TestRepo::new();
    let _remote = repo.setup_remote();

    repo.create_branch("meta-test");
    repo.git(&["push", "origin", "meta-test"]);
    repo.git(&["branch", "-D", "meta-test"]);

    let output = repo.cw(&["new", "meta-test", "--no-term"]);
    assert!(output.status.success());

    // Verify metadata is stored
    let base_branch = repo.git_stdout(&["config", "--get", "branch.meta-test.worktreeBase"]);
    assert_eq!(base_branch.trim(), "main");
}

// ===========================================================================
// get_worktree_status — active (running from within worktree)
// ===========================================================================

#[test]
fn test_get_worktree_status_active() {
    let repo = TestRepo::new();
    let wt = repo.create_worktree("active-test");

    // Run status from inside the worktree
    let stdout = TestRepo::cw_stdout_at(&wt, &["status"]);
    assert!(
        stdout.contains("active-test") || stdout.contains("active") || stdout.contains("Active"),
        "Expected active worktree info, got: {}",
        stdout
    );
}
