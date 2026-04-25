//! `gw guard` — Claude Code hook helper that vets inbound Bash tool calls.
//!
//! Input format: a JSON object with at least `tool_name` and `tool_input`.
//! For Bash, `tool_input.command` is matched against a small risk pattern
//! list. If the command is risky AND the cwd looks unhealthy (missing or
//! not a directory), the helper exits non-zero with a stderr message,
//! causing Claude Code to refuse the tool call.

use std::io::Read;
use std::path::Path;

use crate::error::{CwError, Result};

const RISK_PATTERNS: &[&str] = &[
    "git push",
    "gh release",
    "gh pr merge",
    "npm publish",
    "cargo publish",
    "bun publish",
    "pnpm publish",
];

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

fn command_is_risky(cmd: &str) -> bool {
    let normalized = cmd.split_ascii_whitespace().collect::<Vec<_>>().join(" ");
    RISK_PATTERNS.iter().any(|pat| normalized.contains(pat))
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
    let command = input.get("command").and_then(|v| v.as_str()).unwrap_or("");
    if !command_is_risky(command) {
        return Ok(());
    }
    let cwd_str = input.get("cwd").and_then(|v| v.as_str());
    let cwd = match cwd_str {
        Some(s) => std::path::PathBuf::from(s),
        None => std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
    };
    if !cwd_is_healthy(&cwd) {
        eprintln!(
            "gw guard: blocking risky command '{}' — cwd '{}' does not exist or is not a directory.",
            command,
            cwd.display(),
        );
        return Err(CwError::ExitCode(2));
    }
    Ok(())
}
