//! Plugin install layout / idempotency tests. Each test overrides $HOME
//! via env to keep them hermetic. Marked #[serial] because env vars are
//! process-global and would race across parallel-test workers.

use serial_test::serial;
use std::path::PathBuf;

fn run_install_with_home(home: &std::path::Path) {
    std::env::set_var("HOME", home);
    git_worktree_manager::operations::setup_claude::setup_claude().unwrap();
}

#[test]
#[serial]
fn install_creates_manifest_and_two_skills() {
    let home = tempfile::tempdir().unwrap();
    run_install_with_home(home.path());

    let plugin = home.path().join(".claude").join("plugins").join("gw");
    assert!(plugin.join("plugin.json").exists(), "manifest missing");
    assert!(
        plugin.join("skills").join("delegate").join("SKILL.md").exists(),
        "delegate SKILL.md missing"
    );
    assert!(
        plugin.join("skills").join("manage").join("SKILL.md").exists(),
        "manage SKILL.md missing"
    );
}

#[test]
#[serial]
fn install_is_idempotent() {
    let home = tempfile::tempdir().unwrap();
    run_install_with_home(home.path());
    let manifest = home.path().join(".claude/plugins/gw/plugin.json");
    let mtime_1 = std::fs::metadata(&manifest).unwrap().modified().unwrap();

    std::thread::sleep(std::time::Duration::from_millis(20));
    run_install_with_home(home.path());
    let mtime_2 = std::fs::metadata(&manifest).unwrap().modified().unwrap();

    assert_eq!(mtime_1, mtime_2, "second install must not rewrite unchanged content");
}

#[test]
#[serial]
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
    assert!(body.contains("Worktree-Health Rulebook"), "rulebook section");
    assert!(body.contains("Recommended-Hooks Catalog"), "hooks catalog section");
    assert!(body.contains("Rule: Stale cwd"), "rule 1 header");
    assert!(body.contains("Rule: Wrong-base branching"), "rule 2 header");
    assert!(body.contains("Rule: Sibling worktree drift"), "rule 3 header");
    assert!(body.contains("Rule: Test/lint convention gap"), "rule 4 header");
    assert!(body.contains("Hook 1") && body.contains("SessionStart"), "hook 1");
    assert!(body.contains("Hook 2") && body.contains("PreToolUse"), "hook 2");
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
