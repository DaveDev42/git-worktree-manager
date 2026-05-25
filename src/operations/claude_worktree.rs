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
    /// Branch / worktree identifier Claude Code proposes (e.g.
    /// `agent-<hash>` or a derived branch name). This is the only field the
    /// hook needs to drive `gw new <branch>`; `gw` sanitizes it. Current
    /// Claude Code sends `worktree_name`; some builds emit `name`, so we
    /// accept either. (A payload carrying *both* keys would be rejected as a
    /// duplicate field, but no Claude Code build sends both.)
    #[serde(alias = "name")]
    worktree_name: String,
    /// Commit / ref the worktree should be based on. Optional — `gw new`
    /// falls back to the configured default base when absent. Current Claude
    /// Code sends `base_commit`; older drafts used `base_ref`, so we accept
    /// either.
    #[serde(alias = "base_ref")]
    base_commit: Option<String>,
    /// Legacy/optional explicit path. Claude Code's WorktreeCreate payload
    /// does NOT carry this (the hook is responsible for creating the worktree
    /// and reporting its path), but we honor it if a build ever sends one so
    /// the worktree lands exactly where requested.
    worktree_path: Option<String>,
    /// Repo the hook fired in. Used to relocate the inner `gw new` so it
    /// discovers the right repo root.
    cwd: Option<String>,
    // All other fields (hook_event_name, session_id, transcript_path,
    // isolation_type, …) are intentionally ignored.
    #[serde(flatten)]
    _other: serde_json::Value,
}

#[derive(serde::Deserialize)]
struct RemovePayload {
    worktree_path: String,
    // All other fields (worktree_name, cwd, session_id, …) are intentionally
    // ignored.
    #[serde(flatten)]
    _other: serde_json::Value,
}

// ---------------------------------------------------------------------------
// _claude-worktree-create
// ---------------------------------------------------------------------------

/// Handle a Claude Code `WorktreeCreate` hook event.
///
/// Claude Code's payload carries `worktree_name` (and `cwd`/`base_commit`) but
/// NOT a worktree path — the hook itself is responsible for creating the
/// worktree and reporting where it landed. We read the payload from stdin,
/// delegate to `gw new --emit json --term skip` (self-reexec), parse the
/// resulting JSON, and print only the created `worktree_path` as a plain-text
/// line to stdout — exactly what Claude Code's hook contract expects.
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

    // 3. Use the proposed worktree name as the branch. `gw new` sanitizes it
    //    and chooses the worktree path via its default convention unless an
    //    explicit `worktree_path` was supplied (legacy/optional).
    let branch = payload.worktree_name.trim();
    if branch.is_empty() {
        return Err(CwError::Other(
            "WorktreeCreate hook payload had an empty worktree_name/name".into(),
        ));
    }

    // 4. Re-exec the current binary as `gw new <branch> [--path <path>] [--base
    //    <commit>] --emit json --term skip`. stdout is piped so we can extract
    //    `worktree_path`; stderr is inherited so the user sees human-readable
    //    progress from the inner process.
    let exe = std::env::current_exe()?;
    let mut args = vec!["new".to_string(), branch.to_string()];
    if let Some(path) = payload.worktree_path.as_deref().filter(|p| !p.is_empty()) {
        args.push("--path".to_string());
        args.push(path.to_string());
    }
    if let Some(base) = payload.base_commit.as_deref().filter(|b| !b.is_empty()) {
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
