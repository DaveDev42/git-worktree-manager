//! Inline `--settings` JSON for `gw`-spawned Claude sessions.
//!
//! Claude Code's `--settings <json>` flag merges its content into the user's
//! existing settings layers (user/project/local). Array-valued keys such as
//! `hooks` are concatenated and deduplicated across scopes, so injecting a
//! single PreToolUse(Bash) hook here adds the gw guard without disturbing
//! anything the user already has configured.

use std::path::PathBuf;

use serde_json::json;

use crate::error::{CwError, Result};
use crate::operations::spawn_spec::quote_path_for_shell;

/// Build the `--settings` JSON payload that registers `gw guard` as a
/// PreToolUse(Bash) hook.
///
/// The hook command invokes the **currently-running** binary (resolved via
/// `current_exe()`) so multi-install setups (Homebrew + cargo) call back
/// into the same `gw` that spawned the session. `CW_SPAWN_AI_BIN` overrides
/// the path — used by integration tests to point at a cargo-built binary.
pub fn guard_settings_json() -> Result<String> {
    let self_exe = resolve_self_exe()?;
    let command = format!("{} guard --tool-input -", quote_path_for_shell(&self_exe));

    let payload = json!({
        "hooks": {
            "PreToolUse": [
                {
                    "matcher": "Bash",
                    "hooks": [
                        { "type": "command", "command": command }
                    ]
                }
            ]
        }
    });

    Ok(payload.to_string())
}

fn resolve_self_exe() -> Result<PathBuf> {
    if let Some(s) = std::env::var_os("CW_SPAWN_AI_BIN") {
        return Ok(PathBuf::from(s));
    }
    std::env::current_exe().map_err(|e| {
        CwError::Other(format!(
            "claude_settings: cannot resolve current executable path: {}. \
             Set CW_SPAWN_AI_BIN to override.",
            e
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operations::test_env::{env_lock, EnvGuard};
    use serde_json::Value;

    fn parse(json: &str) -> Value {
        serde_json::from_str(json).expect("valid json")
    }

    #[test]
    fn payload_has_pretooluse_bash_hook() {
        let _lock = env_lock();
        let _guard = EnvGuard::capture(&["CW_SPAWN_AI_BIN"]);
        std::env::set_var("CW_SPAWN_AI_BIN", "/usr/local/bin/gw");

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
        assert!(cmd.ends_with(" guard --tool-input -"));
        assert!(cmd.contains("/usr/local/bin/gw"));
    }

    #[test]
    fn cw_spawn_ai_bin_overrides_path() {
        let _lock = env_lock();
        let _guard = EnvGuard::capture(&["CW_SPAWN_AI_BIN"]);
        std::env::set_var("CW_SPAWN_AI_BIN", "/tmp/custom/gw");

        let s = guard_settings_json().expect("ok");
        let v = parse(&s);
        let cmd = v["hooks"]["PreToolUse"][0]["hooks"][0]["command"]
            .as_str()
            .expect("str");
        assert!(cmd.contains("/tmp/custom/gw"));
    }

    #[test]
    fn path_with_spaces_is_quoted() {
        let _lock = env_lock();
        let _guard = EnvGuard::capture(&["CW_SPAWN_AI_BIN"]);
        std::env::set_var("CW_SPAWN_AI_BIN", "/tmp/dir with space/gw");

        let s = guard_settings_json().expect("ok");
        let v = parse(&s);
        let cmd = v["hooks"]["PreToolUse"][0]["hooks"][0]["command"]
            .as_str()
            .expect("str");
        assert!(cmd.starts_with("\"/tmp/dir with space/gw\""));
    }

    #[test]
    fn windows_backslash_path_is_quoted() {
        let _lock = env_lock();
        let _guard = EnvGuard::capture(&["CW_SPAWN_AI_BIN"]);
        std::env::set_var("CW_SPAWN_AI_BIN", r"C:\Program Files\gw\gw.exe");

        let s = guard_settings_json().expect("ok");
        let v = parse(&s);
        let cmd = v["hooks"]["PreToolUse"][0]["hooks"][0]["command"]
            .as_str()
            .expect("str");
        assert!(cmd.starts_with("\"C:\\Program Files\\gw\\gw.exe\""));
    }
}
