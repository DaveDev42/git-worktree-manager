//! Integration tests for `gw rm` auto-unlock of stale `(pid N)` locks.
//!
//! Scenario covered:
//! - A worktree is locked with `(pid <impossibly-large>)` so the PID is
//!   guaranteed not to be alive. `gw rm` must succeed, leaving an info notice.
//! - A worktree is locked with the *current* test PID. `gw rm` must refuse
//!   to unlock and surface a clearer error.
//! - A worktree is locked with a free-form reason (no `(pid N)` marker).
//!   `gw rm` must NOT auto-unlock; the original git error path is preserved.

mod common;
use common::TestRepo;

/// Lock a worktree with the given reason via raw `git worktree lock --reason`.
fn lock_with_reason(repo: &TestRepo, worktree: &str, reason: &str) {
    repo.git(&["worktree", "lock", "--reason", reason, worktree]);
}

#[test]
fn rm_auto_unlocks_when_pid_is_dead() {
    let repo = TestRepo::new();
    let wt_path = repo.create_worktree("stale");
    let wt_str = wt_path.to_string_lossy().to_string();

    // PID 99_999_999 is well outside the typical max-pid range on Linux
    // (default 32768) and macOS (default 99999), so it is virtually certain
    // to be dead at test time.
    lock_with_reason(&repo, &wt_str, "agent (pid 99999999)");

    let out = repo.cw(&["rm", "stale"]);
    assert!(
        out.status.success(),
        "gw rm should auto-unlock stale lock; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Info notice must mention the stale unlock.
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("stale lock") && stderr.contains("99999999"),
        "expected stale-lock info notice in stderr, got: {stderr}"
    );

    // Worktree must be gone.
    let list = repo.cw_stdout(&["list"]);
    assert!(
        !list.contains("stale"),
        "worktree still listed after rm: {list}"
    );
}

#[test]
fn rm_refuses_when_pid_is_alive() {
    let repo = TestRepo::new();
    let wt_path = repo.create_worktree("live");
    let wt_str = wt_path.to_string_lossy().to_string();

    // Use our own PID — guaranteed to be alive during the test.
    let live_pid = std::process::id();
    let reason = format!("agent (pid {live_pid})");
    lock_with_reason(&repo, &wt_str, &reason);

    let out = repo.cw(&["rm", "live"]);
    assert!(
        !out.status.success(),
        "gw rm should fail when locking PID is alive"
    );

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    assert!(
        combined.contains("still running") || combined.contains(&live_pid.to_string()),
        "expected clearer live-PID error message, got: {combined}"
    );

    // Worktree must still exist (no destructive action when PID is alive).
    let list = repo.cw_stdout(&["list"]);
    assert!(
        list.contains("live"),
        "worktree must remain when PID is alive: {list}"
    );

    // Clean up so the test harness can drop the temp dir without leaving
    // a locked worktree behind.
    repo.git(&["worktree", "unlock", &wt_str]);
}

#[test]
fn rm_does_not_auto_unlock_when_reason_has_no_pid() {
    let repo = TestRepo::new();
    let wt_path = repo.create_worktree("manual");
    let wt_str = wt_path.to_string_lossy().to_string();

    // Free-form reason without `(pid N)` — represents a user's deliberate
    // `git worktree lock --reason "WIP"`. We must NOT auto-unlock.
    lock_with_reason(&repo, &wt_str, "WIP");

    let out = repo.cw(&["rm", "manual"]);
    assert!(
        !out.status.success(),
        "gw rm should fail when lock reason has no (pid N) marker"
    );

    let list = repo.cw_stdout(&["list"]);
    assert!(
        list.contains("manual"),
        "worktree must remain when reason has no pid: {list}"
    );

    repo.git(&["worktree", "unlock", &wt_str]);
}
