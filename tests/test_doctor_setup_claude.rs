//! Smoke test that the doctor's setup-claude block uses the new
//! terminology. We can't easily intercept stdout in a unit test without
//! rearchitecting diagnostics, so this test asserts on the source string
//! constants exposed for tests.

use git_worktree_manager::operations::diagnostics;

#[test]
fn doctor_setup_claude_messages_present() {
    // Helper exposed by diagnostics for tests; see Task 8 step 3.
    let msgs = diagnostics::setup_claude_doctor_messages();
    assert!(
        msgs.installed.contains("plugin installed"),
        "installed message"
    );
    assert!(
        msgs.legacy.contains("Re-run") && msgs.legacy.contains("setup-claude"),
        "legacy upgrade tip"
    );
    assert!(msgs.missing.contains("setup-claude"), "missing install tip");
}
