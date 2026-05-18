//! `gw setup-claude` — project-local one-click installer for Claude Code
//! integration.
//!
//! Writes skill files into `<repo>/.claude/skills/{gw-delegate,gw-manage}/`
//! and registers three Claude Code hooks (PreToolUse Bash guard,
//! WorktreeCreate, WorktreeRemove) in `<repo>/.claude/settings.json`.
//! Idempotent — re-running only writes files whose content changed.

use std::path::Path;

use console::style;

use crate::error::{CwError, Result};
use crate::git;
use crate::operations::sync_claude;

mod skill_delegate;
mod skill_manage;

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

/// Production entry point.
pub fn setup_claude() -> Result<()> {
    let repo_root = git::get_repo_root(None).map_err(|_| {
        CwError::Other(
            "setup-claude: not inside a git repository. Run from within a git repo.".to_string(),
        )
    })?;

    let mut wrote_any = false;
    wrote_any |= write_skill_files(&repo_root)?;
    wrote_any |= sync_hooks(&repo_root)?;

    print_outcome(&repo_root, wrote_any);
    Ok(())
}

/// Returns true if any skill file was written/updated. A file is only
/// rewritten when its on-disk content differs from the embedded content.
fn write_skill_files(repo_root: &Path) -> Result<bool> {
    let mut changed = false;
    let triples = [
        (
            ".claude/skills/gw-delegate/SKILL.md",
            skill_delegate::content(),
        ),
        (".claude/skills/gw-manage/SKILL.md", skill_manage::content()),
        (
            ".claude/skills/gw-manage/references/gw-commands.md",
            skill_manage::reference_content(),
        ),
    ];
    for (rel, content) in triples {
        let path = repo_root.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                CwError::Other(format!(
                    "setup-claude: failed to create {}: {}",
                    parent.display(),
                    e
                ))
            })?;
        }
        let current = std::fs::read_to_string(&path).ok();
        if current.as_deref() != Some(content) {
            std::fs::write(&path, content).map_err(|e| {
                CwError::Other(format!(
                    "setup-claude: failed to write {}: {}",
                    path.display(),
                    e
                ))
            })?;
            changed = true;
        }
    }
    Ok(changed)
}

/// Returns true if `.claude/settings.json` was updated (i.e. at least one
/// hook was newly merged in).
fn sync_hooks(repo_root: &Path) -> Result<bool> {
    let claude_dir = repo_root.join(".claude");
    let settings_path = claude_dir.join("settings.json");
    std::fs::create_dir_all(&claude_dir).map_err(|e| {
        CwError::Other(format!(
            "setup-claude: failed to create {}: {}",
            claude_dir.display(),
            e
        ))
    })?;

    let mut settings: serde_json::Value = if settings_path.exists() {
        let raw = std::fs::read_to_string(&settings_path).map_err(|e| {
            CwError::Other(format!(
                "setup-claude: failed to read {}: {}",
                settings_path.display(),
                e
            ))
        })?;
        serde_json::from_str(&raw).map_err(|e| {
            CwError::Other(format!(
                "setup-claude: malformed JSON in {}: {}. Fix the file manually before re-running.",
                settings_path.display(),
                e
            ))
        })?
    } else {
        serde_json::Value::Object(serde_json::Map::new())
    };

    let changed = sync_claude::merge_hooks_into(&mut settings)?;
    if changed {
        let mut out = serde_json::to_string_pretty(&settings).map_err(|e| {
            CwError::Other(format!("setup-claude: failed to serialize settings: {}", e))
        })?;
        out.push('\n');
        std::fs::write(&settings_path, &out).map_err(|e| {
            CwError::Other(format!(
                "setup-claude: failed to write {}: {}",
                settings_path.display(),
                e
            ))
        })?;
    }
    Ok(changed)
}

/// Returns true if both skill files exist under the repo root's
/// `.claude/skills/gw-*/`.
pub fn is_installed_in_repo(repo_root: &Path) -> bool {
    repo_root
        .join(".claude/skills/gw-delegate/SKILL.md")
        .exists()
        && repo_root.join(".claude/skills/gw-manage/SKILL.md").exists()
}

/// Back-compat alias for `gw doctor`. Resolves repo root and checks installation.
pub fn is_installed() -> bool {
    if let Ok(root) = git::get_repo_root(None) {
        is_installed_in_repo(&root)
    } else {
        false
    }
}

fn print_outcome(repo_root: &Path, wrote_any: bool) {
    let location = repo_root.join(".claude");
    if wrote_any {
        println!(
            "{} Claude Code integration installed at {}.",
            style("*").green().bold(),
            style(location.display()).dim()
        );
    } else {
        println!(
            "{} Claude Code integration already up to date.",
            style("*").green()
        );
    }
    println!(
        "  Skills: {} {}",
        style("gw-delegate").cyan(),
        style("gw-manage").cyan()
    );
    println!("  Hooks: PreToolUse(Bash), WorktreeCreate, WorktreeRemove");
    println!("  Re-run `gw setup-claude` after upgrading gw to refresh skills/hooks.");
}
