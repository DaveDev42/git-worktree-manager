//! Inline `--settings` JSON for `gw`-spawned Claude sessions, and the entry
//! builders used by `gw setup-claude` to register `.claude/settings.json` hooks.
//!
//! Hook commands use the bare name `gw` rather than an absolute path. Claude
//! Code resolves the command through the user's `PATH`, which is what makes
//! `.claude/settings.json` portable across machines (Homebrew, cargo, manual
//! installers, CI) and safe to commit to a repo.

use serde_json::json;

use crate::error::Result;

const GW_BIN: &str = "gw";

/// Build the `--settings` JSON payload that registers `gw guard` as a
/// PreToolUse(Bash) hook.
pub fn guard_settings_json() -> Result<String> {
    let payload = json!({
        "hooks": {
            "PreToolUse": [
                pre_tool_use_bash_entry()?
            ]
        }
    });

    Ok(payload.to_string())
}

/// One PreToolUse(Bash) hook entry — used by both `--settings` inline injection
/// and `setup-claude`'s `.claude/settings.json` merge.
pub fn pre_tool_use_bash_entry() -> Result<serde_json::Value> {
    Ok(json!({
        "matcher": "Bash",
        "hooks": [
            { "type": "command", "command": format!("{GW_BIN} guard --tool-input -") }
        ]
    }))
}

/// One WorktreeCreate hook entry (no matcher — Claude Code does not support
/// matchers for worktree lifecycle events).
pub fn worktree_create_entry() -> Result<serde_json::Value> {
    Ok(json!({
        "hooks": [
            { "type": "command", "command": format!("{GW_BIN} _claude-worktree-create") }
        ]
    }))
}

/// One WorktreeRemove hook entry (no matcher — Claude Code does not support
/// matchers for worktree lifecycle events).
pub fn worktree_remove_entry() -> Result<serde_json::Value> {
    Ok(json!({
        "hooks": [
            { "type": "command", "command": format!("{GW_BIN} _claude-worktree-remove") }
        ]
    }))
}

/// True if `command` looks like a previous `gw setup-claude` registration
/// — any string that ends with one of our known hook suffixes. Used by
/// `sync_claude` to migrate absolute-path entries left by older versions
/// (≤ 0.1.11) to the new bare-name form without creating duplicates.
pub(crate) fn is_legacy_gw_command(command: &str, suffix: &str) -> bool {
    if !command.ends_with(suffix) {
        return false;
    }
    // Avoid matching the current bare-name form (e.g. "gw guard …") — that's
    // not legacy, it's the same string we'd insert this run.
    let prefix = command.trim_end_matches(suffix).trim_end();
    !prefix.is_empty() && prefix != GW_BIN
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn parse(json: &str) -> Value {
        serde_json::from_str(json).expect("valid json")
    }

    #[test]
    fn payload_has_pretooluse_bash_hook() {
        let s = guard_settings_json().expect("ok");
        let v = parse(&s);
        let arr = v["hooks"]["PreToolUse"]
            .as_array()
            .expect("PreToolUse array");
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["matcher"], "Bash");
        let inner = arr[0]["hooks"].as_array().expect("inner hooks");
        assert_eq!(inner.len(), 1);
        assert_eq!(inner[0]["type"], "command");
        let cmd = inner[0]["command"].as_str().expect("command str");
        assert_eq!(cmd, "gw guard --tool-input -");
    }

    #[test]
    fn pre_tool_use_bash_entry_is_bare_gw() {
        let entry = pre_tool_use_bash_entry().expect("ok");
        assert_eq!(entry["matcher"], "Bash");
        let inner = entry["hooks"].as_array().expect("inner hooks array");
        assert_eq!(inner[0]["type"], "command");
        assert_eq!(inner[0]["command"], "gw guard --tool-input -");
    }

    #[test]
    fn worktree_create_entry_is_bare_gw() {
        let entry = worktree_create_entry().expect("ok");
        // WorktreeCreate entries must NOT have a matcher key.
        assert!(entry.get("matcher").is_none());
        let inner = entry["hooks"].as_array().expect("inner hooks array");
        assert_eq!(inner[0]["type"], "command");
        assert_eq!(inner[0]["command"], "gw _claude-worktree-create");
    }

    #[test]
    fn worktree_remove_entry_is_bare_gw() {
        let entry = worktree_remove_entry().expect("ok");
        // WorktreeRemove entries must NOT have a matcher key.
        assert!(entry.get("matcher").is_none());
        let inner = entry["hooks"].as_array().expect("inner hooks array");
        assert_eq!(inner[0]["type"], "command");
        assert_eq!(inner[0]["command"], "gw _claude-worktree-remove");
    }

    #[test]
    fn is_legacy_recognizes_absolute_paths() {
        assert!(is_legacy_gw_command(
            "/usr/local/bin/gw guard --tool-input -",
            " guard --tool-input -",
        ));
        assert!(is_legacy_gw_command(
            "/opt/homebrew/bin/gw _claude-worktree-create",
            " _claude-worktree-create",
        ));
        assert!(is_legacy_gw_command(
            "/Users/dave/proj/target/release/gw _claude-worktree-remove",
            " _claude-worktree-remove",
        ));
    }

    #[test]
    fn is_legacy_rejects_current_bare_form() {
        // Our current form is what we'd insert this run — not legacy.
        assert!(!is_legacy_gw_command(
            "gw guard --tool-input -",
            " guard --tool-input -",
        ));
        assert!(!is_legacy_gw_command(
            "gw _claude-worktree-create",
            " _claude-worktree-create",
        ));
    }

    #[test]
    fn is_legacy_rejects_unrelated_commands() {
        assert!(!is_legacy_gw_command(
            "/usr/local/bin/some-other-tool guard --tool-input -",
            " _claude-worktree-create", // wrong suffix
        ));
        assert!(!is_legacy_gw_command("", " guard --tool-input -"));
    }
}
