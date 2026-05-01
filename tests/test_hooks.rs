//! Tests for hook execution.

mod common;
use common::TestRepo;
use std::process::Command;

/// Verify that run_event executes the configured command when the hook is set.
#[test]
fn run_event_executes_configured_command() {
    let repo = TestRepo::new();
    let marker = repo.path().join(".gw-marker");
    let cfg = format!(
        r#"{{"hooks":{{"post_new":"touch '{}'"}}}}"#,
        marker.display()
    );
    std::fs::write(repo.path().join(".cwconfig.json"), cfg).unwrap();

    // run_event with cwd = repo root, which has .cwconfig.json with post_new set.
    let result = git_worktree_manager::hooks::run_event("post_new", repo.path());
    assert!(result.is_ok(), "hook should succeed: {:?}", result);
    assert!(
        marker.exists(),
        "post_new hook should have touched the marker"
    );
}

#[test]
fn run_event_no_op_for_unknown_event() {
    let repo = TestRepo::new();
    let result = git_worktree_manager::hooks::run_event("definitely-not-an-event", repo.path());
    assert!(result.is_ok(), "unknown event should be a no-op");
}

#[test]
fn run_event_no_op_when_hook_unset() {
    let repo = TestRepo::new();
    // No .cwconfig.json — both post_new and pre_rm are unset.
    let result = git_worktree_manager::hooks::run_event("post_new", repo.path());
    assert!(result.is_ok(), "unset hook should be a no-op");
}

#[test]
fn run_event_propagates_nonzero_exit() {
    let repo = TestRepo::new();
    std::fs::write(
        repo.path().join(".cwconfig.json"),
        r#"{"hooks":{"post_new":"exit 7"}}"#,
    )
    .unwrap();
    let result = git_worktree_manager::hooks::run_event("post_new", repo.path());
    assert!(result.is_err(), "non-zero hook exit should error");
}

/// Regression test: config is found even when `cwd` is a sibling worktree directory.
///
/// Default worktree layout is `../<repo>-<branch>` — a sibling of the main repo,
/// not a child. Walking up from the worktree path would never find the main repo's
/// `.cwconfig.json`. `run_event` must resolve to the main repo root via
/// `get_main_repo_root` before loading config.
#[test]
fn run_event_finds_main_repo_config_from_worktree() {
    let repo = TestRepo::new();

    // A separate tempdir for the marker file so its path doesn't depend on
    // either the main repo or the worktree.
    let marker_dir = tempfile::tempdir().expect("Failed to create marker tempdir");
    let marker = marker_dir.path().join("hook-fired");

    // Write .cwconfig.json with a post_new hook at the main repo root.
    let cfg = format!(
        r#"{{"hooks":{{"post_new":"touch '{}'"}}}}"#,
        marker.display()
    );
    std::fs::write(repo.path().join(".cwconfig.json"), cfg).unwrap();

    // Create a real sibling worktree via `git worktree add --detach`.
    let wt_dir = tempfile::tempdir().expect("Failed to create worktree tempdir");
    let wt_path = wt_dir.path().join("sibling-wt");
    let status = Command::new("git")
        .args([
            "worktree",
            "add",
            "--detach",
            wt_path.to_str().unwrap(),
            "HEAD",
        ])
        .current_dir(repo.path())
        .status()
        .expect("Failed to run git worktree add");
    assert!(status.success(), "git worktree add --detach failed");

    // run_event with cwd = sibling worktree (NOT the main repo).
    // The fix ensures config is still loaded from the main repo root.
    let result = git_worktree_manager::hooks::run_event("post_new", &wt_path);
    assert!(
        result.is_ok(),
        "hook should succeed from sibling worktree: {:?}",
        result
    );
    assert!(
        marker.exists(),
        "post_new hook should have created the marker file"
    );
}
