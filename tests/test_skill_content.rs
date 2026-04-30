//! Content-presence assertions for the bundled Claude Code plugin skills.
//!
//! These tests do not coerce prose; they only lock in the section / rule
//! headers that the spec at
//! docs/superpowers/specs/2026-04-30-plugin-state-verification-design.md
//! mandates. If a header is renamed, update both the skill body and the
//! corresponding assertion together.

use git_worktree_manager::operations::setup_claude::{
    delegate_skill_content_for_test, manage_skill_content_for_test,
};

#[test]
fn delegate_skill_has_step_1_5_pre_flight_check() {
    let content = delegate_skill_content_for_test();
    assert!(
        content.contains("### Step 1.5: Pre-flight check"),
        "delegate skill must contain '### Step 1.5: Pre-flight check' header"
    );
}

#[test]
fn delegate_skill_pre_flight_covers_three_subchecks() {
    let content = delegate_skill_content_for_test();
    for header in [
        "#### A.1 Stacked-base resolution",
        "#### A.2 Duplicate-task detection",
        "#### A.3 Stale-cwd handling",
    ] {
        assert!(
            content.contains(header),
            "delegate skill must contain '{header}' under Step 1.5"
        );
    }
}

#[test]
fn delegate_skill_cross_references_manage_for_destructive_ops() {
    let content = delegate_skill_content_for_test();
    assert!(
        content.contains("don't-kill-busy-siblings"),
        "delegate skill must reference the don't-kill-busy-siblings rule \
         from the manage skill"
    );
}

#[test]
fn manage_skill_has_busy_sibling_rule() {
    let content = manage_skill_content_for_test();
    assert!(
        content.contains("### Rule: Don't kill busy siblings"),
        "manage skill must contain the new busy-sibling rule header"
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
