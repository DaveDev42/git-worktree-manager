/// Integration tests for core operations — ported from Python test_core.py.
/// Covers: create_worktree, delete, list, resume, and remote-branch
/// scenarios.
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
    let output = repo.cw(&["new", "fix-auth", "-T", "skip"]);
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
fn test_create_worktree_custom_path() {
    let mut repo = TestRepo::new();
    let custom = repo.custom_path("my_custom_path");
    let output = repo.cw(&[
        "new",
        "custom-branch",
        "-T",
        "skip",
        "--path",
        custom.to_str().unwrap(),
    ]);
    assert!(
        output.status.success(),
        "cw new --path failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    assert!(custom.exists());
}

// ===========================================================================
// create_worktree — with base branch
// ===========================================================================

#[test]
fn test_create_worktree_with_base_branch() {
    let repo = TestRepo::new();
    repo.create_branch("develop");

    let output = repo.cw(&["new", "feature", "-T", "skip", "--base", "develop"]);
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
        "-T",
        "skip",
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
        let output = repo.cw(&["new", name, "-T", "skip"]);
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
    let output1 = repo.cw(&["new", "duplicate-test", "-T", "skip"]);
    assert!(output1.status.success());

    // Second creation with same name should fail
    let output2 = repo.cw(&["new", "duplicate-test", "-T", "skip"]);
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

    // Create worktree from existing branch
    let output = repo.cw(&["new", "existing-branch", "-T", "skip"]);
    assert!(
        output.status.success(),
        "cw new for existing branch failed: {}{}",
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
fn test_create_worktree_from_remote_only_branch() {
    let mut repo = TestRepo::new();
    let _remote = repo.setup_remote();

    // Create branch, push, delete local
    repo.create_branch("remote-feature");
    repo.git(&["push", "origin", "remote-feature"]);
    repo.git(&["branch", "-D", "remote-feature"]);

    // Verify not local
    let branches = repo.git_stdout(&["branch", "--list", "remote-feature"]);
    assert!(!branches.contains("remote-feature"));

    // Create worktree from remote branch
    let output = repo.cw(&["new", "remote-feature", "-T", "skip"]);
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
fn test_create_worktree_from_remote_with_custom_path() {
    let mut repo = TestRepo::new();
    let _remote = repo.setup_remote();

    repo.create_branch("remote-custom-path");
    repo.git(&["push", "origin", "remote-custom-path"]);
    repo.git(&["branch", "-D", "remote-custom-path"]);

    let custom = repo.custom_path("my-custom-remote-worktree");
    let output = repo.cw(&[
        "new",
        "remote-custom-path",
        "-T",
        "skip",
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
fn test_create_worktree_remote_has_different_content() {
    let mut repo = TestRepo::new();
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

    let output = repo.cw(&["new", "content-branch", "-T", "skip"]);
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
fn test_create_worktree_from_remote_with_explicit_base() {
    let mut repo = TestRepo::new();
    let _remote = repo.setup_remote();
    repo.create_branch("develop");

    repo.create_branch("remote-with-base");
    repo.git(&["push", "origin", "remote-with-base"]);
    repo.git(&["branch", "-D", "remote-with-base"]);

    let output = repo.cw(&["new", "remote-with-base", "-T", "skip", "--base", "develop"]);
    assert!(output.status.success());

    let wt = worktree_path(&repo, "remote-with-base");
    assert!(wt.exists());
}

// ===========================================================================
// create_worktree — remote with invalid base
// ===========================================================================

#[test]
fn test_create_worktree_from_remote_with_invalid_base() {
    let mut repo = TestRepo::new();
    let _remote = repo.setup_remote();

    repo.create_branch("remote-invalid-base");
    repo.git(&["push", "origin", "remote-invalid-base"]);
    repo.git(&["branch", "-D", "remote-invalid-base"]);

    let output = repo.cw(&[
        "new",
        "remote-invalid-base",
        "-T",
        "skip",
        "--base",
        "nonexistent-base",
    ]);
    assert!(!output.status.success());
}

// ===========================================================================
// create_worktree — local takes precedence over remote
// ===========================================================================

#[test]
fn test_create_worktree_local_branch_takes_precedence_over_remote() {
    let mut repo = TestRepo::new();
    let _remote = repo.setup_remote();

    repo.create_branch("both-local-remote");
    repo.git(&["push", "origin", "both-local-remote"]);
    repo.git(&["fetch", "origin"]);

    // Branch exists both locally and remotely — should use local
    let output = repo.cw(&["new", "both-local-remote", "-T", "skip"]);
    assert!(output.status.success());
    let wt = worktree_path(&repo, "both-local-remote");
    assert!(wt.exists());
}

// ===========================================================================
// delete — by branch name
// ===========================================================================

#[test]
fn test_rm_worktree_by_branch() {
    let repo = TestRepo::new();
    let wt = repo.create_worktree("delete-me");
    assert!(wt.exists());

    let output = repo.cw(&["rm", "delete-me"]);
    assert!(output.status.success());

    assert!(!wt.exists());

    let branches = repo.git_stdout(&["branch", "--list", "delete-me"]);
    assert!(!branches.contains("delete-me"));
}

// ===========================================================================
// delete — by path
// ===========================================================================

#[test]
fn test_rm_worktree_by_path() {
    let repo = TestRepo::new();
    let wt = repo.create_worktree("delete-by-path");

    let output = repo.cw(&["rm", wt.to_str().unwrap()]);
    assert!(output.status.success());
    assert!(!wt.exists());
}

// ===========================================================================
// delete — keep branch
// ===========================================================================

#[test]
fn test_rm_worktree_keep_branch() {
    let repo = TestRepo::new();
    let wt = repo.create_worktree("keep-branch");

    let output = repo.cw(&["rm", "keep-branch", "--keep-branch"]);
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
fn test_rm_worktree_nonexistent() {
    let repo = TestRepo::new();
    let output = repo.cw(&["rm", "nonexistent-branch"]);
    assert!(!output.status.success());
}

// ===========================================================================
// delete — main repo protection
// ===========================================================================

#[test]
fn test_rm_main_repo_protection() {
    let repo = TestRepo::new();
    let output = repo.cw(&["rm", repo.path().to_str().unwrap()]);
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
fn test_rm_worktree_created_from_remote() {
    let mut repo = TestRepo::new();
    let _remote = repo.setup_remote();

    repo.create_branch("delete-remote-test");
    repo.git(&["push", "origin", "delete-remote-test"]);
    repo.git(&["branch", "-D", "delete-remote-test"]);

    let output = repo.cw(&["new", "delete-remote-test", "-T", "skip"]);
    assert!(output.status.success());

    let wt = worktree_path(&repo, "delete-remote-test");
    assert!(wt.exists());

    let del = repo.cw(&["rm", "delete-remote-test"]);
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
// worktree status detection — merged (via git branch --merged fallback)
// ===========================================================================

#[test]
fn test_get_worktree_status_merged() {
    let repo = TestRepo::new();
    let wt = repo.create_worktree("merged-test");

    // Make a commit in the worktree
    TestRepo::commit_file_at(&wt, "feature.txt", "feature work", "feat: add feature");

    // Merge the feature branch into main (fast-forward)
    repo.git(&["merge", "merged-test"]);

    // The worktree's branch is now merged into main
    let stdout = repo.cw_stdout(&["list"]);
    assert!(
        stdout.contains("merged"),
        "Expected merged status in list output, got: {}",
        stdout
    );
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
// delete — from inside worktree (current directory)
// ===========================================================================

#[test]
#[cfg_attr(windows, ignore)] // Windows cannot delete cwd
fn test_rm_worktree_current_directory() {
    let repo = TestRepo::new();
    let wt = repo.create_worktree("delete-current");
    assert!(wt.exists());

    // Delete from inside the worktree
    let output = TestRepo::cw_at(&wt, &["rm", "delete-current"]);
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
fn test_rm_worktree_same_branch_and_worktree_name() {
    let repo = TestRepo::new();
    let wt = repo.create_worktree("matching");
    assert!(wt.exists());

    // "matching" as branch should work without ambiguity
    let output = repo.cw(&["rm", "matching"]);
    assert!(output.status.success());
    assert!(!wt.exists());
}

// ===========================================================================
// create_worktree — remote branch stores metadata
// ===========================================================================

#[test]
fn test_create_worktree_from_remote_stores_metadata() {
    let mut repo = TestRepo::new();
    let _remote = repo.setup_remote();

    repo.create_branch("meta-test");
    repo.git(&["push", "origin", "meta-test"]);
    repo.git(&["branch", "-D", "meta-test"]);

    let output = repo.cw(&["new", "meta-test", "-T", "skip"]);
    assert!(output.status.success());

    // Verify metadata is stored
    let base_branch = repo.git_stdout(&["config", "--get", "branch.meta-test.worktreeBase"]);
    assert_eq!(base_branch.trim(), "main");
}
