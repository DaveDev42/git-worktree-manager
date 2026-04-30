//! Local-marketplace installer for Claude Code integration.
//!
//! `gw setup-claude` writes a self-contained Claude Code marketplace tree
//! under the OS data-local dir (e.g. `~/.local/share/git-worktree-manager/
//! claude-marketplace/` on Linux/macOS, `%LOCALAPPDATA%\git-worktree-
//! manager\claude-marketplace\` on Windows). After writing, we shell out
//! to the `claude` CLI to register the marketplace and install/update the
//! plugin so Claude Code actually loads it.
//!
//! Re-runs are idempotent: file content is content-addressed, and the
//! `claude` CLI calls switch between fresh install and update based on a
//! sentinel marker we drop on first run.

use std::path::{Path, PathBuf};

use console::style;

use crate::constants::home_dir_or_fallback;
use crate::error::{CwError, Result};

pub mod claude_cli;
pub mod command_gw;
pub mod legacy;
pub mod manifest;
pub mod paths;
mod skill_delegate;
mod skill_manage;
pub mod writer;

use claude_cli::{ClaudeCli, RealClaudeCli};

/// Tracks which branch of the `claude` CLI invocation path was taken.
enum CliOutcome {
    /// `claude` CLI was not available; we wrote files but did not call it.
    NotRun,
    /// We called `marketplace_add` + `plugin_install` (fresh registration).
    AddInstall,
    /// We called `marketplace_update` + `plugin_update` (refresh).
    UpdateUpdate,
}

#[doc(hidden)]
pub fn manage_skill_content_for_test() -> &'static str {
    skill_manage::content()
}

#[doc(hidden)]
pub fn manage_reference_content_for_test() -> &'static str {
    skill_manage::reference_content()
}

#[doc(hidden)]
pub fn delegate_skill_content_for_test() -> &'static str {
    skill_delegate::content()
}

/// True if our marketplace tree exists at the canonical data-local path.
pub fn is_installed() -> bool {
    let dl = data_local_or_fallback();
    paths::sentinel_under(&dl).exists()
}

/// Backward-compat alias used by `gw doctor`. Returns true if either the
/// new install OR a legacy install (any of three layouts) is present.
pub fn is_skill_installed() -> bool {
    is_installed() || legacy::any_legacy_present()
}

/// Backward-compat alias kept for diagnostics.rs. Same meaning as
/// `is_installed()`.
pub fn is_plugin_installed() -> bool {
    is_installed()
}

/// Production entry point: resolves real home + data-local dirs and uses
/// the real `claude` CLI.
pub fn setup_claude() -> Result<()> {
    let home = home_dir_or_fallback();
    let data_local = data_local_or_fallback();
    setup_claude_with_cli(&home, &data_local, &RealClaudeCli)
}

/// Test/composition entry point. Lets callers inject the home root, the
/// data-local root, and a `ClaudeCli` impl independently.
pub fn setup_claude_with_cli(home: &Path, data_local: &Path, cli: &dyn ClaudeCli) -> Result<()> {
    legacy::remove_legacy_installs_under(home);

    // Check whether Claude Code has our plugin registered in its own state
    // file. This is the source of truth for CLI branching: if Claude Code has
    // uninstalled the plugin (e.g. via `claude plugin uninstall`), the sentinel
    // file still exists but the plugin is gone — we must re-run add+install,
    // not update+update, to re-register it.
    let claude_registered = claude_has_plugin_registered(home);

    let any_changed = write_files(data_local)?;

    let cli_outcome = if cli.is_available() {
        if claude_registered {
            // Refresh: pull marketplace source (no-op for local), then
            // bump the cached plugin to the new version if plugin.json
            // changed.
            let _ = cli.marketplace_update(paths::MARKETPLACE_NAME);
            let _ = cli.plugin_update(paths::PLUGIN_SLUG);
            CliOutcome::UpdateUpdate
        } else {
            cli.marketplace_add(&paths::marketplace_root_under(data_local))
                .map_err(|e| {
                    CwError::Other(format!("`claude plugin marketplace add` failed: {e}"))
                })?;
            cli.plugin_install(paths::PLUGIN_SLUG)
                .map_err(|e| CwError::Other(format!("`claude plugin install` failed: {e}")))?;
            CliOutcome::AddInstall
        }
    } else {
        eprintln!(
            "{} `claude` CLI not found on PATH. Files were written but the plugin",
            style("!").yellow()
        );
        eprintln!("  is not registered with Claude Code. Install Claude Code, then run:");
        eprintln!(
            "    claude plugin marketplace add {}",
            paths::marketplace_root_under(data_local).display()
        );
        eprintln!("    claude plugin install {}", paths::PLUGIN_SLUG);
        CliOutcome::NotRun
    };

    print_outcome(data_local, any_changed, cli_outcome);
    Ok(())
}

/// Returns true iff Claude Code's `installed_plugins.json` contains a
/// non-empty entry for our plugin slug (`gw@gw-local`).
///
/// Treats any I/O or parse error as "not registered" so we fall back safely
/// to a fresh add+install rather than silently doing nothing.
fn claude_has_plugin_registered(home: &Path) -> bool {
    let path = paths::installed_plugins_json_under(home);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return false;
    };
    let Ok(json): std::result::Result<serde_json::Value, _> = serde_json::from_str(&text) else {
        return false;
    };
    json.get("plugins")
        .and_then(|p| p.get(paths::PLUGIN_SLUG))
        .and_then(|v| v.as_array())
        .map(|arr| !arr.is_empty())
        .unwrap_or(false)
}

fn write_files(data_local: &Path) -> Result<bool> {
    let mut any_changed = false;
    any_changed |= writer::write_if_changed(
        &paths::marketplace_manifest_under(data_local),
        manifest::marketplace_json(),
    )?;
    any_changed |= writer::write_if_changed(
        &paths::plugin_manifest_under(data_local),
        &manifest::plugin_json(),
    )?;
    any_changed |=
        writer::write_if_changed(&paths::command_gw_under(data_local), command_gw::content())?;
    any_changed |= writer::write_if_changed(
        &paths::skill_delegate_under(data_local),
        skill_delegate::content(),
    )?;
    any_changed |= writer::write_if_changed(
        &paths::skill_manage_under(data_local),
        skill_manage::content(),
    )?;
    any_changed |= writer::write_if_changed(
        &paths::skill_manage_reference_under(data_local),
        skill_manage::reference_content(),
    )?;
    let sentinel = paths::sentinel_under(data_local);
    if !writer::sentinel_present(&sentinel) {
        writer::write_sentinel(&sentinel)?;
        any_changed = true;
    }
    Ok(any_changed)
}

fn print_outcome(data_local: &Path, any_changed: bool, cli_outcome: CliOutcome) {
    let location = paths::marketplace_root_under(data_local);
    if !any_changed {
        match cli_outcome {
            CliOutcome::AddInstall => {
                println!(
                    "{} gw plugin re-registered with Claude Code (files unchanged).",
                    style("*").green()
                );
                println!("  Location: {}", style(location.display()).dim());
            }
            CliOutcome::NotRun | CliOutcome::UpdateUpdate => {
                println!("{} gw plugin already up to date.", style("*").green());
                println!("  Location: {}", style(location.display()).dim());
            }
        }
        return;
    }

    let verb = match cli_outcome {
        CliOutcome::UpdateUpdate => "refreshed",
        CliOutcome::AddInstall | CliOutcome::NotRun => "installed",
    };
    println!(
        "{} gw plugin {} at {}.",
        style("*").green().bold(),
        verb,
        style(location.display()).dim()
    );
    println!(
        "  Use {} in Claude Code to delegate tasks to worktrees.",
        style("/gw").cyan()
    );
    println!(
        "  The bundled '{}' skill recommends hooks (e.g. SessionStart sanity)",
        style("manage").cyan()
    );
    println!("  in-session when relevant. It edits your project's .claude/settings.json");
    println!("  on your consent — gw itself never modifies any settings file.");
}

fn data_local_or_fallback() -> PathBuf {
    dirs::data_local_dir().unwrap_or_else(|| home_dir_or_fallback().join(".local").join("share"))
}
