//! `gw guard` — Claude Code PreToolUse(Bash) hook helper.
//!
//! Input format: a JSON object with at least `tool_name` and `tool_input`.
//! Policy: when `tool_name == "Bash"` and the call's cwd is unhealthy
//! (missing path or not a directory), block the tool call with a strong
//! abort instruction in stderr — regardless of which command is being run.
//!
//! Rationale: the guard exists for sessions whose worktree was removed out
//! from under them (e.g. `gw rm` while Claude was paused). In that state
//! every shell command runs in a stale environment, so the safe default is
//! to refuse all of them and ask the user before continuing. Sessions with
//! a healthy cwd pass through transparently.

use std::io::Read;
use std::path::Path;

use crate::error::{CwError, Result};

#[derive(Debug, serde::Deserialize)]
struct HookPayload {
    tool_name: Option<String>,
    tool_input: Option<serde_json::Value>,
}

fn read_input(source: &str) -> std::io::Result<String> {
    if source == "-" {
        let mut s = String::new();
        std::io::stdin().read_to_string(&mut s)?;
        Ok(s)
    } else {
        std::fs::read_to_string(source)
    }
}

fn cwd_is_healthy(cwd: &Path) -> bool {
    cwd.exists() && cwd.is_dir()
}

pub fn run(tool_input_source: &str) -> Result<()> {
    let raw = read_input(tool_input_source).map_err(CwError::Io)?;
    let payload: HookPayload = serde_json::from_str(&raw).map_err(CwError::Json)?;

    if payload.tool_name.as_deref() != Some("Bash") {
        return Ok(());
    }
    let input = match payload.tool_input {
        Some(v) => v,
        None => return Ok(()),
    };
    let cwd_str = input.get("cwd").and_then(|v| v.as_str());
    let cwd = match cwd_str {
        Some(s) => std::path::PathBuf::from(s),
        None => std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
    };
    if cwd_is_healthy(&cwd) {
        return Ok(());
    }

    let command = input.get("command").and_then(|v| v.as_str()).unwrap_or("");
    eprintln!(
        "STOP. gw guard: cwd '{}' no longer exists or is not a directory. \
         The worktree may have been removed while this session was paused. \
         Do NOT execute any further commands. Ask the user to confirm the \
         situation before continuing. (blocked command: '{}')",
        cwd.display(),
        command,
    );
    Err(CwError::ExitCode(2))
}
