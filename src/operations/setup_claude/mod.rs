//! Plugin installer for Claude Code integration.
//!
//! Installs gw as a Claude Code *plugin* at `~/.claude/plugins/gw/` with
//! two skills (`delegate`, `manage`). Removes legacy single-skill installs
//! at `~/.claude/skills/gw/` and `~/.claude/skills/gw-delegate/`.

use std::path::PathBuf;

use console::style;

use crate::constants::home_dir_or_fallback;
use crate::error::Result;

mod legacy;
mod manifest;
mod skill_delegate;
mod skill_manage;

const PLUGIN_NAME: &str = "gw";

fn plugin_dir() -> PathBuf {
    home_dir_or_fallback()
        .join(".claude")
        .join("plugins")
        .join(PLUGIN_NAME)
}

fn manifest_path() -> PathBuf { plugin_dir().join("plugin.json") }
fn delegate_skill_path() -> PathBuf { plugin_dir().join("skills").join("delegate").join("SKILL.md") }
fn manage_skill_path() -> PathBuf { plugin_dir().join("skills").join("manage").join("SKILL.md") }
fn manage_reference_path() -> PathBuf {
    plugin_dir()
        .join("skills")
        .join("manage")
        .join("references")
        .join("gw-commands.md")
}

/// True if the plugin manifest exists at the canonical path.
pub fn is_plugin_installed() -> bool { manifest_path().exists() }

/// Backward-compatible alias used by `gw doctor`. Returns true if either the
/// new plugin OR a legacy skill install is present.
pub fn is_skill_installed() -> bool {
    is_plugin_installed() || legacy::any_legacy_present()
}

fn write_if_changed(
    path: &PathBuf,
    new_content: &str,
) -> std::result::Result<bool, std::io::Error> {
    if path.exists() {
        let existing = std::fs::read_to_string(path).unwrap_or_default();
        if existing == new_content {
            return Ok(false);
        }
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, new_content)?;
    Ok(true)
}

pub fn setup_claude() -> Result<()> {
    legacy::remove_legacy_installs();

    let manifest = manifest_path();
    let delegate = delegate_skill_path();
    let manage = manage_skill_path();
    let reference = manage_reference_path();

    let mut any_changed = false;
    any_changed |= write_if_changed(&manifest, manifest::content())?;
    any_changed |= write_if_changed(&delegate, skill_delegate::content())?;
    any_changed |= write_if_changed(&manage, skill_manage::content())?;
    any_changed |= write_if_changed(&reference, skill_manage::reference_content())?;

    if !any_changed {
        println!("{} gw plugin already up to date.\n", style("*").green());
        println!("  Location: {}", style(plugin_dir().display()).dim());
        return Ok(());
    }

    println!(
        "{} gw plugin installed at {}.\n",
        style("*").green().bold(),
        style(plugin_dir().display()).dim()
    );
    println!(
        "  Use {} in Claude Code to delegate tasks to worktrees.",
        style("/gw").cyan()
    );
    println!(
        "  The bundled '{}' skill will recommend hooks (e.g. SessionStart sanity)",
        style("manage").cyan()
    );
    println!("  in-session when relevant. It edits your project's .claude/settings.json");
    println!("  on your consent — gw itself never modifies any settings file.\n");

    Ok(())
}
