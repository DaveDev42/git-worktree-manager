//! Internal handlers for Claude Code's `WorktreeCreate` and `WorktreeRemove`
//! hook events.
//!
//! These are invoked via the hidden `_claude-worktree-create` and
//! `_claude-worktree-remove` subcommands. Both read a JSON payload from stdin
//! and write only what Claude Code's hook contract requires to stdout.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::error::{CwError, Result};

// ---------------------------------------------------------------------------
// Payload types
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize)]
struct CreatePayload {
    /// Absolute path where Claude Code wants the worktree created. Claude
    /// derives the trailing segment from a suggested branch name, so we both
    /// honor this exact path (via `gw new --path`) and use its final
    /// component as the branch name.
    worktree_path: String,
    /// Git ref the worktree should be based on (e.g. `main`). Optional —
    /// `gw new` falls back to the configured default base when absent.
    base_ref: Option<String>,
    /// Repo the hook fired in. Used to relocate the inner `gw new` so it
    /// discovers the right repo root.
    cwd: Option<String>,
    // All other fields (hook_event_name, session_id, transcript_path,
    // isolation_type, …) are intentionally ignored.
    #[serde(flatten)]
    _other: serde_json::Value,
}

/// Derive a branch name from the worktree path Claude Code proposes.
///
/// Claude builds the path as `<dir>/<branch-ish-name>`, so the final path
/// component is the branch name. Returns an error if the path has no usable
/// final component.
fn branch_from_worktree_path(worktree_path: &str) -> Result<String> {
    Path::new(worktree_path)
        .file_name()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            CwError::Other(format!(
                "could not derive a branch name from worktree_path '{worktree_path}'"
            ))
        })
}

#[derive(serde::Deserialize)]
struct RemovePayload {
    worktree_path: String,
    // All other fields are intentionally ignored.
    #[serde(flatten)]
    _other: serde_json::Value,
}

// ---------------------------------------------------------------------------
// _claude-worktree-create
// ---------------------------------------------------------------------------

/// Handle a Claude Code `WorktreeCreate` hook event.
///
/// Reads the JSON payload from stdin, delegates to `gw new --emit json
/// --term skip` (self-reexec), parses the resulting JSON, and prints only the
/// `worktree_path` as a plain-text line to stdout — exactly what Claude Code's
/// hook contract expects.
///
/// Human-readable progress output from the inner `gw new` call flows through
/// to stderr unchanged, so the user can see what's happening.
pub fn run_create() -> Result<()> {
    // 1. Read and parse the hook payload from stdin.
    let mut buf = String::new();
    std::io::stdin().read_to_string(&mut buf)?;
    let payload: CreatePayload = serde_json::from_str(&buf)
        .map_err(|e| CwError::Other(format!("invalid WorktreeCreate hook payload: {e}")))?;

    // 2. If cwd is provided, switch to it so the inner `gw new` discovers the
    //    correct repo root (the hook may run from elsewhere).
    if let Some(ref cwd) = payload.cwd {
        std::env::set_current_dir(cwd)?;
    }

    // 3. Derive the branch name from the proposed worktree path, and honor that
    //    exact path so the worktree lands where Claude Code expects it.
    let branch = branch_from_worktree_path(&payload.worktree_path)?;

    // 4. Re-exec the current binary as `gw new <branch> --path <path> [--base
    //    <ref>] --emit json --term skip`. stdout is piped so we can extract
    //    `worktree_path`; stderr is inherited so the user sees human-readable
    //    progress from the inner process.
    let exe = std::env::current_exe()?;
    let mut args = vec![
        "new".to_string(),
        branch,
        "--path".to_string(),
        payload.worktree_path.clone(),
    ];
    if let Some(base) = payload.base_ref.as_deref().filter(|b| !b.is_empty()) {
        args.push("--base".to_string());
        args.push(base.to_string());
    }
    args.extend(["--emit", "json", "--term", "skip"].map(str::to_string));
    let output = Command::new(&exe)
        .args(&args)
        // Force the inner `gw new` to error rather than silently returning an
        // existing worktree path with no JSON on stdout — the hook contract
        // requires that we only succeed when a fresh worktree was created.
        .env("CW_NON_INTERACTIVE", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()?;

    if !output.status.success() {
        return Err(CwError::Other(format!(
            "gw new failed (exit {})",
            output.status.code().unwrap_or(-1)
        )));
    }

    // 5. Parse the JSON line from the inner process and extract worktree_path.
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim())
        .map_err(|e| CwError::Other(format!("could not parse `gw new --emit json` output: {e}")))?;
    let wt_path = parsed["worktree_path"]
        .as_str()
        .ok_or_else(|| CwError::Other("`worktree_path` missing in `gw new` JSON output".into()))?;

    // 6. Emit only the plain-text path — what Claude Code's hook reads.
    println!("{}", wt_path);
    Ok(())
}

// ---------------------------------------------------------------------------
// _claude-worktree-remove
// ---------------------------------------------------------------------------

/// Handle a Claude Code `WorktreeRemove` hook event.
///
/// Reads the JSON payload from stdin, extracts `worktree_path`, and runs the
/// configured `pre_rm` hook in that directory as a best-effort advisory step.
/// Hook failures are logged to stderr but never abort the handler — Claude Code
/// owns the actual removal, and `WorktreeRemove` hooks cannot block it anyway.
///
/// Always exits 0.
pub fn run_remove() -> Result<()> {
    // 1. Read and parse the hook payload from stdin.
    let mut buf = String::new();
    std::io::stdin().read_to_string(&mut buf)?;
    let payload: RemovePayload = serde_json::from_str(&buf)
        .map_err(|e| CwError::Other(format!("invalid WorktreeRemove hook payload: {e}")))?;

    let wt_path = Path::new(&payload.worktree_path);

    // 2. Run pre_rm hook as advisory — log warning on failure, never propagate.
    //    This mirrors the contract established in task #2 (hooks.rs).
    if let Err(e) = crate::hooks::run_event("pre_rm", wt_path) {
        eprintln!(
            "warning: pre_rm hook failed for '{}' (advisory, removal proceeds): {}",
            wt_path.display(),
            e
        );
    }

    // 3. Claude Code performs the actual worktree removal; we have nothing more to do.
    Ok(())
}
