//! Verifies legacy cleanup removes the right directories and is safe.

use git_worktree_manager::operations::setup_claude::legacy;
use std::fs;

#[test]
fn removes_legacy_skill_dirs() {
    let home = tempfile::tempdir().unwrap();
    let a = home.path().join(".claude/skills/gw");
    let b = home.path().join(".claude/skills/gw-delegate");
    fs::create_dir_all(&a).unwrap();
    fs::create_dir_all(&b).unwrap();
    fs::write(a.join("SKILL.md"), b"x").unwrap();
    fs::write(b.join("SKILL.md"), b"x").unwrap();

    legacy::remove_legacy_installs_under(home.path());

    assert!(!a.exists());
    assert!(!b.exists());
}

#[test]
fn removes_broken_plugin_layout() {
    let home = tempfile::tempdir().unwrap();
    let bad = home.path().join(".claude/plugins/gw");
    fs::create_dir_all(bad.join("skills/delegate")).unwrap();
    fs::write(bad.join("plugin.json"), b"{}").unwrap();
    fs::write(bad.join("skills/delegate/SKILL.md"), b"x").unwrap();

    legacy::remove_legacy_installs_under(home.path());

    assert!(!bad.exists(), "broken plugin layout must be removed");
}

#[test]
fn leaves_unrelated_dirs_alone() {
    let home = tempfile::tempdir().unwrap();
    let unrelated_skill = home.path().join(".claude/skills/some-other-skill");
    let unrelated_plugin = home.path().join(".claude/plugins/some-other-plugin");
    fs::create_dir_all(&unrelated_skill).unwrap();
    fs::create_dir_all(&unrelated_plugin).unwrap();
    fs::write(unrelated_skill.join("SKILL.md"), b"x").unwrap();
    fs::write(unrelated_plugin.join("plugin.json"), b"{}").unwrap();

    legacy::remove_legacy_installs_under(home.path());

    assert!(unrelated_skill.exists(), "unrelated skill must survive");
    assert!(unrelated_plugin.exists(), "unrelated plugin must survive");
}

#[test]
fn any_legacy_present_under_detects_each_layout() {
    let home1 = tempfile::tempdir().unwrap();
    fs::create_dir_all(home1.path().join(".claude/skills/gw")).unwrap();
    assert!(legacy::any_legacy_present_under(home1.path()));

    let home2 = tempfile::tempdir().unwrap();
    fs::create_dir_all(home2.path().join(".claude/plugins/gw")).unwrap();
    assert!(legacy::any_legacy_present_under(home2.path()));

    let home3 = tempfile::tempdir().unwrap();
    assert!(!legacy::any_legacy_present_under(home3.path()));
}
