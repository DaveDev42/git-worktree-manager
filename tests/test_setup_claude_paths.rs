//! Verifies path layout for the local-marketplace install. All paths
//! are derived from a `data_local` root so the tests stay hermetic.

use git_worktree_manager::operations::setup_claude::paths;
use std::path::Path;

#[test]
fn marketplace_root_under_data_local() {
    let data_local = Path::new("/tmp/data-local");
    let mp = paths::marketplace_root_under(data_local);
    assert_eq!(
        mp,
        Path::new("/tmp/data-local/git-worktree-manager/claude-marketplace")
    );
}

#[test]
fn marketplace_manifest_path() {
    let data_local = Path::new("/tmp/data-local");
    let mf = paths::marketplace_manifest_under(data_local);
    assert!(mf.ends_with(".claude-plugin/marketplace.json"));
}

#[test]
fn plugin_manifest_path() {
    let data_local = Path::new("/tmp/data-local");
    let mf = paths::plugin_manifest_under(data_local);
    assert!(mf.ends_with("gw-plugin/.claude-plugin/plugin.json"));
}

#[test]
fn command_gw_path() {
    let data_local = Path::new("/tmp/data-local");
    let cmd = paths::command_gw_under(data_local);
    assert!(cmd.ends_with("gw-plugin/commands/gw.md"));
}

#[test]
fn skill_delegate_path() {
    let data_local = Path::new("/tmp/data-local");
    let p = paths::skill_delegate_under(data_local);
    assert!(p.ends_with("gw-plugin/skills/delegate/SKILL.md"));
}

#[test]
fn skill_manage_paths() {
    let data_local = Path::new("/tmp/data-local");
    let main = paths::skill_manage_under(data_local);
    let refs = paths::skill_manage_reference_under(data_local);
    assert!(main.ends_with("gw-plugin/skills/manage/SKILL.md"));
    assert!(refs.ends_with("gw-plugin/skills/manage/references/gw-commands.md"));
}

#[test]
fn sentinel_path_at_marketplace_root() {
    let data_local = Path::new("/tmp/data-local");
    let s = paths::sentinel_under(data_local);
    assert!(s.ends_with("claude-marketplace/.gw-managed"));
}

#[test]
fn marketplace_and_plugin_slug_constants() {
    assert_eq!(paths::MARKETPLACE_NAME, "gw-local");
    assert_eq!(paths::PLUGIN_NAME, "gw");
    assert_eq!(paths::PLUGIN_SLUG, "gw@gw-local");
}
