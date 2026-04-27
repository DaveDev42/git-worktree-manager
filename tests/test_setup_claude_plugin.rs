//! Plugin install layout / idempotency tests. Each test installs into a
//! tempdir via `setup_claude_under(&Path)` to stay hermetic without
//! mutating process-global env vars.

use std::path::PathBuf;

fn run_install_with_home(home: &std::path::Path) {
    git_worktree_manager::operations::setup_claude::setup_claude_under(home).unwrap();
}

#[test]
fn install_creates_manifest_and_two_skills() {
    let home = tempfile::tempdir().unwrap();
    run_install_with_home(home.path());

    let plugin = home.path().join(".claude").join("plugins").join("gw");
    assert!(plugin.join("plugin.json").exists(), "manifest missing");
    assert!(
        plugin
            .join("skills")
            .join("delegate")
            .join("SKILL.md")
            .exists(),
        "delegate SKILL.md missing"
    );
    assert!(
        plugin
            .join("skills")
            .join("manage")
            .join("SKILL.md")
            .exists(),
        "manage SKILL.md missing"
    );
}

#[test]
fn install_is_idempotent() {
    // NOTE: ~/.claude/plugins/gw/ is the *legacy* install location that legacy
    // cleanup now removes on every run (it is the broken plugin layout being
    // replaced by the marketplace install). Idempotency of the new marketplace
    // path is tested in test_setup_claude_marketplace.rs (Task 7+).
    // This test verifies that a second run at minimum leaves the manifest in
    // place (i.e. the re-install after cleanup succeeds without error).
    let home = tempfile::tempdir().unwrap();
    run_install_with_home(home.path());
    let manifest = home.path().join(".claude/plugins/gw/plugin.json");
    assert!(manifest.exists(), "manifest must exist after first install");

    run_install_with_home(home.path());
    assert!(
        manifest.exists(),
        "manifest must exist after second install (legacy cleanup + reinstall)"
    );
}

#[test]
fn install_removes_legacy_skill_dir() {
    let home = tempfile::tempdir().unwrap();
    let legacy = home.path().join(".claude/skills/gw");
    std::fs::create_dir_all(&legacy).unwrap();
    std::fs::write(legacy.join("SKILL.md"), b"old").unwrap();

    run_install_with_home(home.path());
    assert!(!legacy.exists(), "legacy skill directory must be removed");
}

#[test]
fn manage_skill_contains_required_sections() {
    let body = git_worktree_manager::operations::setup_claude::manage_skill_content_for_test();
    assert!(body.contains("name: manage"), "frontmatter name");
    assert!(
        body.contains("Worktree-Health Rulebook"),
        "rulebook section"
    );
    assert!(
        body.contains("Recommended-Hooks Catalog"),
        "hooks catalog section"
    );
    assert!(body.contains("Rule: Stale cwd"), "rule 1 header");
    assert!(body.contains("Rule: Wrong-base branching"), "rule 2 header");
    assert!(
        body.contains("Rule: Sibling worktree drift"),
        "rule 3 header"
    );
    assert!(
        body.contains("Rule: Test/lint convention gap"),
        "rule 4 header"
    );
    assert!(
        body.contains("Hook 1") && body.contains("SessionStart"),
        "hook 1"
    );
    assert!(
        body.contains("Hook 2") && body.contains("PreToolUse"),
        "hook 2"
    );
    assert!(body.contains("Hook 3") && body.contains("Stop"), "hook 3");
}

#[test]
fn manage_reference_contains_command_table() {
    let body = git_worktree_manager::operations::setup_claude::manage_reference_content_for_test();
    assert!(body.contains("gw new"));
    assert!(body.contains("gw delete"));
    assert!(body.contains("gw clean"));
    assert!(body.len() > 1000, "reference content should be substantial");
}

#[allow(dead_code)]
fn _suppress_unused() -> PathBuf {
    PathBuf::new()
}
