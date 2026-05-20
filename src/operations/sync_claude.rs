//! Hook-merge logic for Claude Code settings.json integration.
//!
//! Used by `gw setup-claude` to register WorktreeCreate, WorktreeRemove, and
//! PreToolUse(Bash) hooks in `<repo>/.claude/settings.json` idempotently.
//!
//! Safe to call repeatedly: existing hook entries are detected by exact
//! command-string match and skipped; other user-configured hooks are never
//! touched. Legacy absolute-path entries written by gw ≤ 0.1.11 are migrated
//! to the new bare-name form on the next run.

use serde_json::Value;

use crate::error::{CwError, Result};
use crate::operations::claude_settings;

const GUARD_SUFFIX: &str = " guard --tool-input -";
const CREATE_SUFFIX: &str = " _claude-worktree-create";
const REMOVE_SUFFIX: &str = " _claude-worktree-remove";

/// Idempotently merge the three gw hooks into `settings`.
///
/// Returns `true` if the settings object was modified (i.e. a hook was added
/// or a legacy absolute-path entry was migrated), `false` if every entry was
/// already present in its current bare-name form.
pub(crate) fn merge_hooks_into(settings: &mut Value) -> Result<bool> {
    // The root must be a JSON object.
    let root = settings.as_object_mut().ok_or_else(|| {
        CwError::Other(
            "sync-claude: malformed .claude/settings.json — root is not a JSON object.".to_string(),
        )
    })?;

    // Ensure `hooks` exists and is an object.
    let hooks_entry = root
        .entry("hooks")
        .or_insert_with(|| Value::Object(serde_json::Map::new()));
    let hooks = hooks_entry.as_object_mut().ok_or_else(|| {
        CwError::Other(
            "sync-claude: malformed .claude/settings.json — `hooks` is not a JSON object."
                .to_string(),
        )
    })?;

    let mut changed = false;

    // --- PreToolUse(Bash) ---
    changed |= merge_one_hook(
        hooks,
        "PreToolUse",
        claude_settings::pre_tool_use_bash_entry()?,
        GUARD_SUFFIX,
        true,
    )?;

    // --- WorktreeCreate ---
    changed |= merge_one_hook(
        hooks,
        "WorktreeCreate",
        claude_settings::worktree_create_entry()?,
        CREATE_SUFFIX,
        false,
    )?;

    // --- WorktreeRemove ---
    changed |= merge_one_hook(
        hooks,
        "WorktreeRemove",
        claude_settings::worktree_remove_entry()?,
        REMOVE_SUFFIX,
        false,
    )?;

    Ok(changed)
}

/// Merge a single hook entry into `hooks[event_key]`, dropping any legacy
/// absolute-path registrations with the given `legacy_suffix` first.
///
/// When `bash_matcher` is true, both the legacy-drop and presence-check only
/// consider entries whose `matcher` field equals `"Bash"`, so that unrelated
/// hook types with a different matcher are untouched.
///
/// Returns `true` if the hooks object was modified.
fn merge_one_hook(
    hooks: &mut serde_json::Map<String, Value>,
    event_key: &str,
    entry: Value,
    legacy_suffix: &str,
    bash_matcher: bool,
) -> Result<bool> {
    let our_cmd = extract_first_command(&entry);
    let arr = hooks
        .entry(event_key)
        .or_insert_with(|| Value::Array(vec![]))
        .as_array_mut()
        .ok_or_else(|| {
            CwError::Other(format!("sync-claude: `hooks.{event_key}` is not an array."))
        })?;

    let mut changed = false;
    if bash_matcher {
        if drop_legacy_bash_entries(arr, legacy_suffix) {
            changed = true;
        }
        if !pre_tool_use_bash_already_present(arr, our_cmd.as_deref()) {
            arr.push(entry);
            changed = true;
        }
    } else {
        if drop_legacy_entries(arr, legacy_suffix) {
            changed = true;
        }
        if !command_already_present(arr, our_cmd.as_deref()) {
            arr.push(entry);
            changed = true;
        }
    }
    Ok(changed)
}

/// Extract the command string from the first inner hook of an entry, if any.
fn extract_first_command(entry: &Value) -> Option<String> {
    entry["hooks"]
        .as_array()?
        .first()?
        .get("command")?
        .as_str()
        .map(|s| s.to_string())
}

/// Drop PreToolUse entries whose matcher is "Bash" and whose first inner
/// command looks like a legacy absolute-path `gw guard …` registration.
/// Returns true if any entry was removed.
fn drop_legacy_bash_entries(arr: &mut Vec<Value>, suffix: &str) -> bool {
    let before = arr.len();
    arr.retain(|item| {
        if item.get("matcher").and_then(|m| m.as_str()) != Some("Bash") {
            return true;
        }
        let cmd = match first_command(item) {
            Some(c) => c,
            None => return true,
        };
        !claude_settings::is_legacy_gw_command(cmd, suffix)
    });
    arr.len() != before
}

/// Drop WorktreeCreate/WorktreeRemove entries whose first inner command looks
/// like a legacy absolute-path registration. Returns true if any entry was
/// removed.
fn drop_legacy_entries(arr: &mut Vec<Value>, suffix: &str) -> bool {
    let before = arr.len();
    arr.retain(|item| {
        let cmd = match first_command(item) {
            Some(c) => c,
            None => return true,
        };
        !claude_settings::is_legacy_gw_command(cmd, suffix)
    });
    arr.len() != before
}

fn first_command(entry: &Value) -> Option<&str> {
    entry["hooks"].as_array()?.first()?.get("command")?.as_str()
}

/// Check whether the given command string already appears in any entry's inner
/// hooks inside `arr` (for PreToolUse with a Bash matcher specifically).
fn pre_tool_use_bash_already_present(arr: &[Value], our_cmd: Option<&str>) -> bool {
    let our_cmd = match our_cmd {
        Some(c) => c,
        None => return false,
    };
    arr.iter().any(|item| {
        if item.get("matcher").and_then(|m| m.as_str()) != Some("Bash") {
            return false;
        }
        command_in_inner_hooks(item, our_cmd)
    })
}

/// Check whether `our_cmd` appears in the inner hooks of any entry in `arr`.
/// Used for WorktreeCreate / WorktreeRemove (no matcher).
fn command_already_present(arr: &[Value], our_cmd: Option<&str>) -> bool {
    let our_cmd = match our_cmd {
        Some(c) => c,
        None => return false,
    };
    arr.iter().any(|item| command_in_inner_hooks(item, our_cmd))
}

/// Return true if `cmd` appears in the `hooks[].command` of `entry`.
fn command_in_inner_hooks(entry: &Value, cmd: &str) -> bool {
    entry["hooks"]
        .as_array()
        .map(|inner| {
            inner
                .iter()
                .any(|h| h.get("command").and_then(|c| c.as_str()) == Some(cmd))
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn merge_into_empty_object_returns_changed() {
        let mut v = json!({});
        let changed = merge_hooks_into(&mut v).expect("ok");
        assert!(changed, "empty → should be changed");

        let hooks = &v["hooks"];
        assert_eq!(hooks["PreToolUse"].as_array().unwrap().len(), 1);
        assert_eq!(hooks["WorktreeCreate"].as_array().unwrap().len(), 1);
        assert_eq!(hooks["WorktreeRemove"].as_array().unwrap().len(), 1);

        // Sanity-check: hooks are emitted with the bare `gw` form.
        let pre_cmd = hooks["PreToolUse"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap();
        assert_eq!(pre_cmd, "gw guard --tool-input -");
    }

    #[test]
    fn merge_twice_is_idempotent() {
        let mut v = json!({});
        merge_hooks_into(&mut v).expect("first");
        let changed2 = merge_hooks_into(&mut v).expect("second");
        assert!(!changed2, "second run should be no-op");

        let hooks = &v["hooks"];
        assert_eq!(hooks["PreToolUse"].as_array().unwrap().len(), 1);
        assert_eq!(hooks["WorktreeCreate"].as_array().unwrap().len(), 1);
        assert_eq!(hooks["WorktreeRemove"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn merge_preserves_existing_user_hooks() {
        // A user-defined PreToolUse hook with matcher=Write.
        let mut v = json!({
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": "Write",
                        "hooks": [
                            { "type": "command", "command": "/usr/local/bin/my-lint" }
                        ]
                    }
                ]
            }
        });
        merge_hooks_into(&mut v).expect("ok");
        let arr = v["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(arr.len(), 2);
        let matchers: Vec<&str> = arr
            .iter()
            .filter_map(|e| e.get("matcher").and_then(|m| m.as_str()))
            .collect();
        assert!(matchers.contains(&"Write"));
        assert!(matchers.contains(&"Bash"));
    }

    #[test]
    fn merge_preserves_other_bash_hook_alongside_ours() {
        // Another Bash hook with a different command — must be kept.
        let mut v = json!({
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": "Bash",
                        "hooks": [
                            { "type": "command", "command": "/usr/local/bin/other-guard" }
                        ]
                    }
                ]
            }
        });
        merge_hooks_into(&mut v).expect("ok");
        let arr = v["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(arr.len(), 2);
    }

    #[test]
    fn merge_non_object_root_errors() {
        let mut v = json!([]);
        let err = merge_hooks_into(&mut v).expect_err("should error on array root");
        assert!(format!("{err}").contains("not a JSON object"));
    }

    #[test]
    fn merge_non_object_hooks_key_errors() {
        let mut v = json!({ "hooks": "not-an-object" });
        let err = merge_hooks_into(&mut v).expect_err("should error");
        assert!(format!("{err}").contains("not a JSON object"));
    }

    #[test]
    fn merge_migrates_legacy_absolute_path_entries() {
        // Settings written by gw ≤ 0.1.11 had absolute paths.
        let mut v = json!({
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": "Bash",
                        "hooks": [
                            { "type": "command", "command": "/usr/local/bin/gw guard --tool-input -" }
                        ]
                    }
                ],
                "WorktreeCreate": [
                    {
                        "hooks": [
                            { "type": "command", "command": "/opt/homebrew/bin/gw _claude-worktree-create" }
                        ]
                    }
                ],
                "WorktreeRemove": [
                    {
                        "hooks": [
                            { "type": "command", "command": "/Users/dave/proj/target/release/gw _claude-worktree-remove" }
                        ]
                    }
                ]
            }
        });
        let changed = merge_hooks_into(&mut v).expect("ok");
        assert!(changed, "legacy entries should trigger a write");

        // Exactly one entry per hook type, now in bare-name form.
        let hooks = &v["hooks"];
        assert_eq!(hooks["PreToolUse"].as_array().unwrap().len(), 1);
        assert_eq!(hooks["WorktreeCreate"].as_array().unwrap().len(), 1);
        assert_eq!(hooks["WorktreeRemove"].as_array().unwrap().len(), 1);

        assert_eq!(
            hooks["PreToolUse"][0]["hooks"][0]["command"],
            "gw guard --tool-input -"
        );
        assert_eq!(
            hooks["WorktreeCreate"][0]["hooks"][0]["command"],
            "gw _claude-worktree-create"
        );
        assert_eq!(
            hooks["WorktreeRemove"][0]["hooks"][0]["command"],
            "gw _claude-worktree-remove"
        );
    }

    #[test]
    fn migration_keeps_unrelated_bash_hook() {
        // Legacy gw guard entry + a user's unrelated Bash hook → migrate gw,
        // leave the user's alone.
        let mut v = json!({
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": "Bash",
                        "hooks": [
                            { "type": "command", "command": "/usr/local/bin/gw guard --tool-input -" }
                        ]
                    },
                    {
                        "matcher": "Bash",
                        "hooks": [
                            { "type": "command", "command": "/usr/local/bin/other-guard" }
                        ]
                    }
                ]
            }
        });
        merge_hooks_into(&mut v).expect("ok");
        let arr = v["hooks"]["PreToolUse"].as_array().unwrap();
        // user's other-guard + new bare-gw guard = 2
        assert_eq!(arr.len(), 2);
        let commands: Vec<&str> = arr
            .iter()
            .map(|e| e["hooks"][0]["command"].as_str().unwrap())
            .collect();
        assert!(commands.contains(&"gw guard --tool-input -"));
        assert!(commands.contains(&"/usr/local/bin/other-guard"));
    }
}
