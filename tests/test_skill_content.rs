//! Content-presence assertions for the bundled Claude Code skills.
//!
//! These tests do not coerce prose; they only lock in section / rule headers
//! that the skill bodies are expected to contain. If a header is renamed,
//! update both the skill body and the corresponding assertion together.

use git_worktree_manager::operations::setup_claude::{
    delegate_skill_content_for_test, manage_reference_content_for_test,
    manage_skill_content_for_test,
};

#[test]
fn delegate_skill_has_when_to_delegate_section() {
    let content = delegate_skill_content_for_test();
    assert!(
        content.contains("## When to delegate vs. work in-session"),
        "delegate skill must contain 'When to delegate vs. work in-session' section"
    );
}

#[test]
fn delegate_skill_uses_agent_isolation_worktree() {
    let content = delegate_skill_content_for_test();
    assert!(
        content.contains("Agent") && content.contains("isolation"),
        "delegate skill must reference Agent isolation: worktree delegation model"
    );
}

#[test]
fn delegate_skill_cross_references_manage_for_destructive_ops() {
    let content = delegate_skill_content_for_test();
    assert!(
        content.contains("manage"),
        "delegate skill must reference the manage skill for cleanup/destructive ops"
    );
}

#[test]
fn delegate_skill_has_branch_naming_section() {
    let content = delegate_skill_content_for_test();
    assert!(
        content.contains("## Branch naming"),
        "delegate skill must contain a 'Branch naming' section"
    );
}

#[test]
fn manage_skill_has_busy_sibling_rule() {
    let content = manage_skill_content_for_test();
    assert!(
        content.contains("### Rule: Don't kill busy siblings"),
        "manage skill must contain the busy-sibling rule header"
    );
}

#[test]
fn manage_skill_busy_sibling_rule_warns_against_force() {
    let content = manage_skill_content_for_test();
    let rule_start = content
        .find("### Rule: Don't kill busy siblings")
        .expect("busy-sibling rule header must be present");
    // Look only inside the rule body, up to the next "### " heading.
    let after = &content[rule_start..];
    let rule_end = after[3..].find("\n### ").map_or(after.len(), |p| p + 3);
    let rule_body = &after[..rule_end];
    assert!(
        rule_body.contains("--force"),
        "busy-sibling rule must mention --force explicitly so the model \
         knows what NOT to volunteer"
    );
}

#[test]
fn manage_skill_has_hook_integration_section() {
    let content = manage_skill_content_for_test();
    assert!(
        content.contains("## 3. Hook integration"),
        "manage skill must contain a hook integration section"
    );
}

#[test]
fn manage_skill_references_setup_claude_not_sync_claude() {
    let content = manage_skill_content_for_test();
    assert!(
        content.contains("gw setup-claude"),
        "manage skill must reference 'gw setup-claude'"
    );
    assert!(
        !content.contains("gw sync-claude"),
        "manage skill must not reference the removed 'gw sync-claude' command"
    );
}

#[test]
fn delegate_skill_references_setup_claude_not_sync_claude() {
    let content = delegate_skill_content_for_test();
    assert!(
        !content.contains("gw sync-claude"),
        "delegate skill must not reference the removed 'gw sync-claude' command"
    );
}

#[test]
fn reference_content_has_setup_claude_section() {
    let content = manage_reference_content_for_test();
    assert!(
        content.contains("### `gw setup-claude`"),
        "reference content must have a 'gw setup-claude' section"
    );
    assert!(
        !content.contains("### `gw sync-claude`"),
        "reference content must not have the removed 'gw sync-claude' section"
    );
}
