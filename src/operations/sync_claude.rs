//! `gw sync-claude` — register WorktreeCreate, WorktreeRemove, and
//! PreToolUse(Bash) hooks in `<repo>/.claude/settings.json` idempotently.
//!
//! Safe to re-run: existing hook entries are detected by exact command-string
//! match and skipped; other user-configured hooks are never touched.

use console::style;
use serde_json::Value;

use crate::error::{CwError, Result};
use crate::git;
use crate::operations::claude_settings;

/// Entry point for `gw sync-claude`.
pub fn run() -> Result<()> {
    let repo_root = git::get_repo_root(None).map_err(|_| {
        CwError::Other(
            "sync-claude: not inside a git repository. \
             Run this command from within a git repo."
                .to_string(),
        )
    })?;

    let claude_dir = repo_root.join(".claude");
    let settings_path = claude_dir.join("settings.json");

    // Ensure the .claude directory exists.
    if !claude_dir.exists() {
        std::fs::create_dir_all(&claude_dir).map_err(|e| {
            CwError::Other(format!(
                "sync-claude: failed to create {}: {}",
                claude_dir.display(),
                e
            ))
        })?;
    }

    // Read existing settings or start with an empty object.
    let mut settings: Value = if settings_path.exists() {
        let raw = std::fs::read_to_string(&settings_path).map_err(|e| {
            CwError::Other(format!(
                "sync-claude: failed to read {}: {}",
                settings_path.display(),
                e
            ))
        })?;
        serde_json::from_str(&raw).map_err(|e| {
            CwError::Other(format!(
                "sync-claude: malformed JSON in {}: {}. \
                 Fix the file manually before re-running.",
                settings_path.display(),
                e
            ))
        })?
    } else {
        Value::Object(serde_json::Map::new())
    };

    let changed = merge_hooks_into(&mut settings)?;

    if changed {
        // Pretty-print with 2-space indent, trailing newline.
        let mut out = serde_json::to_string_pretty(&settings).map_err(|e| {
            CwError::Other(format!("sync-claude: failed to serialize settings: {}", e))
        })?;
        out.push('\n');
        std::fs::write(&settings_path, &out).map_err(|e| {
            CwError::Other(format!(
                "sync-claude: failed to write {}: {}",
                settings_path.display(),
                e
            ))
        })?;
        println!(
            "{} {}",
            style("synced").green().bold(),
            style(settings_path.display().to_string()).dim()
        );
        println!(
            "  {} PreToolUse(Bash) guard, WorktreeCreate, WorktreeRemove hooks registered.",
            style("✓").green()
        );
    } else {
        println!(
            "{} {}",
            style("already up to date:").cyan().bold(),
            style(settings_path.display().to_string()).dim()
        );
        println!(
            "  {} All gw hooks are already present — nothing to change.",
            style("✓").green()
        );
    }

    Ok(())
}

/// Idempotently merge the three gw hooks into `settings`.
///
/// Returns `true` if the settings object was modified (i.e. at least one hook
/// was appended), `false` if every entry was already present.
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
    if hooks_entry.as_object().is_none() {
        return Err(CwError::Other(
            "sync-claude: malformed .claude/settings.json — `hooks` is not a JSON object."
                .to_string(),
        ));
    }
    let hooks = hooks_entry.as_object_mut().expect("just checked");

    let mut changed = false;

    // --- PreToolUse(Bash) ---
    {
        let entry = claude_settings::pre_tool_use_bash_entry()?;
        let our_cmd = extract_first_command(&entry);
        let arr = hooks
            .entry("PreToolUse")
            .or_insert_with(|| Value::Array(vec![]))
            .as_array_mut()
            .ok_or_else(|| {
                CwError::Other("sync-claude: `hooks.PreToolUse` is not an array.".to_string())
            })?;

        if !pre_tool_use_bash_already_present(arr, our_cmd.as_deref()) {
            arr.push(entry);
            changed = true;
        }
    }

    // --- WorktreeCreate ---
    {
        let entry = claude_settings::worktree_create_entry()?;
        let our_cmd = extract_first_command(&entry);
        let arr = hooks
            .entry("WorktreeCreate")
            .or_insert_with(|| Value::Array(vec![]))
            .as_array_mut()
            .ok_or_else(|| {
                CwError::Other("sync-claude: `hooks.WorktreeCreate` is not an array.".to_string())
            })?;

        if !command_already_present(arr, our_cmd.as_deref()) {
            arr.push(entry);
            changed = true;
        }
    }

    // --- WorktreeRemove ---
    {
        let entry = claude_settings::worktree_remove_entry()?;
        let our_cmd = extract_first_command(&entry);
        let arr = hooks
            .entry("WorktreeRemove")
            .or_insert_with(|| Value::Array(vec![]))
            .as_array_mut()
            .ok_or_else(|| {
                CwError::Other("sync-claude: `hooks.WorktreeRemove` is not an array.".to_string())
            })?;

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

/// Check whether the given command string already appears in any entry's inner
/// hooks inside `arr` (for PreToolUse with a Bash matcher specifically).
///
/// We use exact string equality. The command is built from the current self-exe,
/// so a path change would insert a new entry rather than silently shadowing the
/// old one — intentional, and the user can clean up stale entries manually.
fn pre_tool_use_bash_already_present(arr: &[Value], our_cmd: Option<&str>) -> bool {
    let our_cmd = match our_cmd {
        Some(c) => c,
        None => return false,
    };
    arr.iter().any(|item| {
        // Must have matcher == "Bash"
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
    use crate::operations::test_env::{env_lock, EnvGuard};
    use serde_json::json;

    #[test]
    fn merge_into_empty_object_returns_changed() {
        let _lock = env_lock();
        let _guard = EnvGuard::capture(&["CW_SPAWN_AI_BIN"]);
        std::env::set_var("CW_SPAWN_AI_BIN", "/usr/local/bin/gw");

        let mut v = json!({});
        let changed = merge_hooks_into(&mut v).expect("ok");
        assert!(changed, "empty → should be changed");

        let hooks = &v["hooks"];
        assert!(hooks["PreToolUse"].as_array().unwrap().len() == 1);
        assert!(hooks["WorktreeCreate"].as_array().unwrap().len() == 1);
        assert!(hooks["WorktreeRemove"].as_array().unwrap().len() == 1);
    }

    #[test]
    fn merge_twice_is_idempotent() {
        let _lock = env_lock();
        let _guard = EnvGuard::capture(&["CW_SPAWN_AI_BIN"]);
        std::env::set_var("CW_SPAWN_AI_BIN", "/usr/local/bin/gw");

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
        let _lock = env_lock();
        let _guard = EnvGuard::capture(&["CW_SPAWN_AI_BIN"]);
        std::env::set_var("CW_SPAWN_AI_BIN", "/usr/local/bin/gw");

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
        // User's Write hook + our Bash hook = 2
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
        let _lock = env_lock();
        let _guard = EnvGuard::capture(&["CW_SPAWN_AI_BIN"]);
        std::env::set_var("CW_SPAWN_AI_BIN", "/usr/local/bin/gw");

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
        // other-guard entry + ours = 2
        assert_eq!(arr.len(), 2);
    }

    #[test]
    fn merge_non_object_root_errors() {
        let _lock = env_lock();
        let _guard = EnvGuard::capture(&["CW_SPAWN_AI_BIN"]);
        std::env::set_var("CW_SPAWN_AI_BIN", "/usr/local/bin/gw");

        let mut v = json!([]);
        let err = merge_hooks_into(&mut v).expect_err("should error on array root");
        assert!(format!("{err}").contains("not a JSON object"));
    }

    #[test]
    fn merge_non_object_hooks_key_errors() {
        let _lock = env_lock();
        let _guard = EnvGuard::capture(&["CW_SPAWN_AI_BIN"]);
        std::env::set_var("CW_SPAWN_AI_BIN", "/usr/local/bin/gw");

        let mut v = json!({ "hooks": "not-an-object" });
        let err = merge_hooks_into(&mut v).expect_err("should error");
        assert!(format!("{err}").contains("not a JSON object"));
    }
}
