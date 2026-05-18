//! Pins the strings that `gw doctor` prints for its Claude Code section.
//! Because `check_claude_integration` is not easily interceptable without
//! rearchitecting diagnostics, we assert on the constants in
//! `setup_claude_doctor_messages()`, which is the single source of truth
//! that the print sites now read directly. A refactor that silently drops
//! or scrambles these strings will break this test.

use git_worktree_manager::operations::diagnostics;

#[test]
fn doctor_setup_claude_messages_present() {
    let msgs = diagnostics::setup_claude_doctor_messages();
    assert!(
        msgs.installed.contains("skills installed"),
        "installed message: got {:?}",
        msgs.installed
    );
    assert!(
        msgs.missing_alert.contains("Claude Code detected"),
        "missing alert line"
    );
    assert!(
        msgs.missing_tip.contains("setup-claude") && msgs.missing_tip.contains("install"),
        "missing install tip"
    );
}
