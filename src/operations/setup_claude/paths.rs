//! Filesystem layout for the local-marketplace install.
//!
//! All helpers take a `data_local` root (typically `dirs::data_local_dir()`)
//! so tests can pass a tempdir without touching real user state.

use std::path::{Path, PathBuf};

pub const MARKETPLACE_NAME: &str = "gw-local";
pub const PLUGIN_NAME: &str = "gw";
pub const PLUGIN_SLUG: &str = "gw@gw-local";

const APP_DIR: &str = "git-worktree-manager";
const MARKETPLACE_SUBDIR: &str = "claude-marketplace";
const PLUGIN_SUBDIR: &str = "gw-plugin";
const SENTINEL_FILE: &str = ".gw-managed";

pub fn marketplace_root_under(data_local: &Path) -> PathBuf {
    data_local.join(APP_DIR).join(MARKETPLACE_SUBDIR)
}

pub fn marketplace_manifest_under(data_local: &Path) -> PathBuf {
    marketplace_root_under(data_local)
        .join(".claude-plugin")
        .join("marketplace.json")
}

pub fn plugin_root_under(data_local: &Path) -> PathBuf {
    marketplace_root_under(data_local).join(PLUGIN_SUBDIR)
}

pub fn plugin_manifest_under(data_local: &Path) -> PathBuf {
    plugin_root_under(data_local)
        .join(".claude-plugin")
        .join("plugin.json")
}

pub fn command_gw_under(data_local: &Path) -> PathBuf {
    plugin_root_under(data_local).join("commands").join("gw.md")
}

pub fn skill_delegate_under(data_local: &Path) -> PathBuf {
    plugin_root_under(data_local)
        .join("skills")
        .join("delegate")
        .join("SKILL.md")
}

pub fn skill_manage_under(data_local: &Path) -> PathBuf {
    plugin_root_under(data_local)
        .join("skills")
        .join("manage")
        .join("SKILL.md")
}

pub fn skill_manage_reference_under(data_local: &Path) -> PathBuf {
    plugin_root_under(data_local)
        .join("skills")
        .join("manage")
        .join("references")
        .join("gw-commands.md")
}

pub fn sentinel_under(data_local: &Path) -> PathBuf {
    marketplace_root_under(data_local).join(SENTINEL_FILE)
}
