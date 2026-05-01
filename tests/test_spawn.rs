//! Integration tests for `gw spawn` / `spawn_in_worktree`.

mod common;
use common::TestRepo;

use std::sync::Mutex;

use git_worktree_manager::operations::ai_tools::spawn_in_worktree;

/// Mutex to serialize env-var mutations so parallel test threads don't stomp
/// on each other's CW_AI_TOOL / CW_LAUNCH_METHOD values.
static ENV_MUTEX: Mutex<()> = Mutex::new(());

/// Execute `f` with `CW_LAUNCH_METHOD=foreground` and `CW_AI_TOOL=true`
/// (the no-op binary), then restore the previous values.
fn with_noop_ai<F: FnOnce()>(f: F) {
    let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    let saved_launch = std::env::var("CW_LAUNCH_METHOD").ok();
    let saved_tool = std::env::var("CW_AI_TOOL").ok();

    std::env::set_var("CW_LAUNCH_METHOD", "foreground");
    std::env::set_var("CW_AI_TOOL", "true");

    f();

    match saved_launch {
        Some(v) => std::env::set_var("CW_LAUNCH_METHOD", v),
        None => std::env::remove_var("CW_LAUNCH_METHOD"),
    }
    match saved_tool {
        Some(v) => std::env::set_var("CW_AI_TOOL", v),
        None => std::env::remove_var("CW_AI_TOOL"),
    }
}

#[test]
fn spawn_in_worktree_launches_in_existing_worktree() {
    let repo = TestRepo::new();
    let wt_path = repo.create_worktree("feat-x");

    with_noop_ai(|| {
        let result = spawn_in_worktree(&wt_path, "feat-x", None);
        assert!(
            result.is_ok(),
            "spawn_in_worktree returned Err: {:?}",
            result
        );
    });
}

#[test]
fn spawn_in_worktree_with_prompt() {
    let repo = TestRepo::new();
    let wt_path = repo.create_worktree("feat-y");

    with_noop_ai(|| {
        let result = spawn_in_worktree(&wt_path, "feat-y", Some("hello"));
        assert!(
            result.is_ok(),
            "spawn_in_worktree with prompt returned Err: {:?}",
            result
        );
    });
}
