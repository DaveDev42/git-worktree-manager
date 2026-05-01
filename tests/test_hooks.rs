//! Tests for hook execution.

mod common;
use common::TestRepo;

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
    assert!(marker.exists(), "post_new hook should have touched the marker");
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
