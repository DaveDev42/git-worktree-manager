/// Tests for the hook system.
/// Ported from tests/test_hooks.py (32 tests).
mod common;

use common::TestRepo;
use git_worktree_manager::hooks;

#[test]
fn test_hook_events_list() {
    assert!(hooks::HOOK_EVENTS.contains(&"worktree.pre_create"));
    assert!(hooks::HOOK_EVENTS.contains(&"worktree.post_create"));
    assert!(hooks::HOOK_EVENTS.contains(&"worktree.pre_delete"));
    assert!(hooks::HOOK_EVENTS.contains(&"worktree.post_delete"));
    assert!(hooks::HOOK_EVENTS.contains(&"merge.pre"));
    assert!(hooks::HOOK_EVENTS.contains(&"merge.post"));
    assert!(hooks::HOOK_EVENTS.contains(&"pr.pre"));
    assert!(hooks::HOOK_EVENTS.contains(&"pr.post"));
    assert!(hooks::HOOK_EVENTS.contains(&"resume.pre"));
    assert!(hooks::HOOK_EVENTS.contains(&"resume.post"));
    assert!(hooks::HOOK_EVENTS.contains(&"sync.pre"));
    assert!(hooks::HOOK_EVENTS.contains(&"sync.post"));
    assert_eq!(hooks::HOOK_EVENTS.len(), 12);
}

#[test]
fn test_hook_add_via_cli() {
    let repo = TestRepo::new();
    let output = repo.cw(&[
        "hook",
        "add",
        "worktree.post_create",
        "echo hello",
        "--id",
        "test-hook",
    ]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("test-hook"));
}

#[test]
fn test_hook_add_with_description() {
    let repo = TestRepo::new();
    let output = repo.cw(&[
        "hook",
        "add",
        "worktree.post_create",
        "npm install",
        "--id",
        "deps",
        "--description",
        "Install dependencies",
    ]);
    assert!(output.status.success());

    let list = repo.cw_stdout(&["hook", "list", "worktree.post_create"]);
    assert!(list.contains("deps"));
    assert!(list.contains("npm install"));
    assert!(list.contains("Install dependencies"));
}

#[test]
fn test_hook_add_invalid_event() {
    let repo = TestRepo::new();
    let output = repo.cw(&["hook", "add", "invalid.event", "echo hello"]);
    assert!(!output.status.success());
}

#[test]
fn test_hook_add_duplicate_id() {
    let repo = TestRepo::new();
    repo.cw(&[
        "hook",
        "add",
        "worktree.post_create",
        "npm install",
        "--id",
        "deps",
    ]);
    let output = repo.cw(&[
        "hook",
        "add",
        "worktree.post_create",
        "npm test",
        "--id",
        "deps",
    ]);
    assert!(!output.status.success());
}

#[test]
fn test_hook_add_multiple() {
    let repo = TestRepo::new();
    repo.cw(&[
        "hook",
        "add",
        "worktree.post_create",
        "npm install",
        "--id",
        "deps",
    ]);
    repo.cw(&[
        "hook",
        "add",
        "worktree.post_create",
        "npm test",
        "--id",
        "test",
    ]);

    let list = repo.cw_stdout(&["hook", "list", "worktree.post_create"]);
    assert!(list.contains("deps"));
    assert!(list.contains("test"));
}

#[test]
fn test_hook_remove() {
    let repo = TestRepo::new();
    repo.cw(&[
        "hook",
        "add",
        "worktree.post_create",
        "npm install",
        "--id",
        "deps",
    ]);
    let output = repo.cw(&["hook", "remove", "worktree.post_create", "deps"]);
    assert!(output.status.success());

    let list = repo.cw_stdout(&["hook", "list", "worktree.post_create"]);
    assert!(!list.contains("deps"));
}

#[test]
fn test_hook_remove_nonexistent() {
    let repo = TestRepo::new();
    let output = repo.cw(&["hook", "remove", "worktree.post_create", "nonexistent"]);
    assert!(!output.status.success());
}

#[test]
fn test_hook_disable() {
    let repo = TestRepo::new();
    repo.cw(&[
        "hook",
        "add",
        "worktree.post_create",
        "npm install",
        "--id",
        "deps",
    ]);
    let output = repo.cw(&["hook", "disable", "worktree.post_create", "deps"]);
    assert!(output.status.success());

    let list = repo.cw_stdout(&["hook", "list", "worktree.post_create"]);
    assert!(list.contains("disabled"));
}

#[test]
fn test_hook_enable() {
    let repo = TestRepo::new();
    repo.cw(&[
        "hook",
        "add",
        "worktree.post_create",
        "npm install",
        "--id",
        "deps",
    ]);
    repo.cw(&["hook", "disable", "worktree.post_create", "deps"]);
    let output = repo.cw(&["hook", "enable", "worktree.post_create", "deps"]);
    assert!(output.status.success());

    let list = repo.cw_stdout(&["hook", "list", "worktree.post_create"]);
    assert!(list.contains("enabled"));
}

#[test]
fn test_hook_enable_nonexistent() {
    let repo = TestRepo::new();
    let output = repo.cw(&["hook", "enable", "worktree.post_create", "nonexistent"]);
    assert!(!output.status.success());
}

#[test]
fn test_hook_list_empty() {
    let repo = TestRepo::new();
    let stdout = repo.cw_stdout(&["hook", "list"]);
    assert!(stdout.contains("No hooks configured"));
}

#[test]
fn test_hook_list_specific_event_empty() {
    let repo = TestRepo::new();
    let stdout = repo.cw_stdout(&["hook", "list", "worktree.post_create"]);
    assert!(stdout.contains("no hooks"));
}

#[test]
fn test_hook_run_dry_run() {
    let repo = TestRepo::new();
    repo.cw(&[
        "hook",
        "add",
        "worktree.post_create",
        "echo hello",
        "--id",
        "test",
    ]);
    let output = repo.cw(&["hook", "run", "worktree.post_create", "--dry-run"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Would run"));
}

#[test]
fn test_hook_run_no_hooks() {
    let repo = TestRepo::new();
    let output = repo.cw(&["hook", "run", "worktree.post_create"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No hooks configured"));
}

#[test]
fn test_hook_run_executes() {
    let repo = TestRepo::new();
    let marker = repo.path().join("hook_ran.txt");
    let cmd = format!("touch {}", marker.display());
    repo.cw(&[
        "hook",
        "add",
        "worktree.post_create",
        &cmd,
        "--id",
        "marker",
    ]);
    let output = repo.cw(&["hook", "run", "worktree.post_create"]);
    assert!(output.status.success());
    assert!(marker.exists(), "Hook should have created marker file");
}

#[test]
fn test_hook_run_skips_disabled() {
    let repo = TestRepo::new();
    let marker = repo.path().join("should_not_exist.txt");
    let cmd = format!("touch {}", marker.display());
    repo.cw(&[
        "hook",
        "add",
        "worktree.post_create",
        &cmd,
        "--id",
        "disabled-hook",
    ]);
    repo.cw(&["hook", "disable", "worktree.post_create", "disabled-hook"]);
    let output = repo.cw(&["hook", "run", "worktree.post_create"]);
    assert!(output.status.success());
    assert!(
        !marker.exists(),
        "Disabled hook should not have created marker"
    );
}

// Test hook config file structure
#[test]
fn test_load_hooks_config_empty() {
    let tmp = tempfile::TempDir::new().unwrap();
    let hooks = hooks::load_hooks_config(Some(tmp.path()));
    assert!(hooks.is_empty());
}

#[test]
fn test_get_hooks_empty() {
    let tmp = tempfile::TempDir::new().unwrap();
    let hooks = hooks::get_hooks("worktree.post_create", Some(tmp.path()));
    assert!(hooks.is_empty());
}
