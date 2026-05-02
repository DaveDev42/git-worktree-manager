//! End-to-end tests for the hook contracts documented in README.md:
//!
//! - `post_new` fires after `gw new` and a non-zero exit surfaces as a
//!   non-zero CLI exit code (the worktree is left on disk because the
//!   hook runs after `git worktree add`, but the failure is reported).
//! - `pre_rm` fires before `gw rm` removes a worktree, including when
//!   `--force` is passed. `--force` only bypasses the busy-detection
//!   gate, not the user's hook.
//!
//! Unix-only: hooks run via `sh -c` and these tests use `touch` / `exit`.
//! Windows lacks `sh` in $PATH for the test repo, so the same contracts
//! are covered at the unit level in `test_hooks.rs` (which doesn't shell
//! out for the cross-platform cases).

#![cfg(unix)]

mod common;
use common::TestRepo;

/// `gw new` must fail (exit != 0) when `post_new` exits non-zero.
/// The worktree itself stays on disk — this is documented as best-effort
/// for `post_new` because the hook fires after `git worktree add`.
#[test]
fn post_new_nonzero_exit_fails_gw_new() {
    let repo = TestRepo::new();
    std::fs::write(
        repo.path().join(".cwconfig.json"),
        r#"{"hooks":{"post_new":"exit 7"}}"#,
    )
    .unwrap();

    let out = repo.cw(&["new", "feat-postnew-fail", "--no-term"]);
    assert!(
        !out.status.success(),
        "gw new should fail when post_new exits non-zero. stdout={:?} stderr={:?}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("post_new"),
        "stderr should mention post_new failure, got: {:?}",
        stderr
    );
}

/// `pre_rm` must fire when `gw rm --force` is used. `--force` bypasses
/// busy detection only — never the user's hook.
#[test]
fn pre_rm_fires_with_force_flag() {
    let repo = TestRepo::new();
    let _ = repo.create_worktree("feat-prerm-force");

    // Marker file in a separate tempdir to avoid the worktree being
    // affected by the hook's side effect.
    let marker_dir = tempfile::tempdir().expect("marker tempdir");
    let marker = marker_dir.path().join("pre_rm-fired");

    let cfg = format!(r#"{{"hooks":{{"pre_rm":"touch '{}'"}}}}"#, marker.display());
    std::fs::write(repo.path().join(".cwconfig.json"), cfg).unwrap();

    let out = repo.cw(&["rm", "feat-prerm-force", "--force"]);
    assert!(
        out.status.success(),
        "gw rm --force should succeed. stdout={:?} stderr={:?}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    assert!(
        marker.exists(),
        "pre_rm hook should have fired even with --force"
    );
}

/// `pre_rm` non-zero exit must abort `gw rm`, with or without `--force`.
#[test]
fn pre_rm_nonzero_exit_aborts_rm_with_force() {
    let repo = TestRepo::new();
    let wt_path = repo.create_worktree("feat-prerm-abort");

    std::fs::write(
        repo.path().join(".cwconfig.json"),
        r#"{"hooks":{"pre_rm":"exit 9"}}"#,
    )
    .unwrap();

    let out = repo.cw(&["rm", "feat-prerm-abort", "--force"]);
    assert!(
        !out.status.success(),
        "gw rm --force should fail when pre_rm exits non-zero"
    );
    assert!(
        wt_path.exists(),
        "worktree dir should still exist after pre_rm aborted the remove"
    );
}
