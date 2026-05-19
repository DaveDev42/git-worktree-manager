//! Lifecycle hooks executed by gw at key worktree transitions.
//!
//! Configured via `hooks.post_new` / `hooks.pre_rm` keys in
//! `~/.config/git-worktree-manager/config.json` or `.cwconfig.json`.

use std::path::Path;
use std::process::Command;

use crate::error::Result;

/// Run the configured hook for `event` (one of `"post_new"`, `"pre_rm"`),
/// resolving the hook command from the layered config rooted at the main
/// repo (resolved from `cwd` via `git::get_main_repo_root`, falling back to
/// `cwd` if that fails). The hook runs with `cwd` as its working directory.
///
/// No-op (returns `Ok(())`) when the event name is unknown or the hook is
/// unset. Hook is run as `sh -c <cmd>`. A non-zero exit propagates as
/// `CwError::Other`.
///
/// **Caller policy** — this function always returns `Err` on non-zero exit;
/// callers decide what that means:
/// - `pre_rm` callers treat the error as **advisory**: log a warning, then
///   continue with removal (aligned with Claude Code's `WorktreeRemove` hook,
///   which cannot block cleanup).
/// - `post_new` callers treat the error as **blocking**: propagate it as a
///   non-zero `gw new` exit code and skip the AI tool launch.
pub fn run_event(event: &str, cwd: &Path) -> Result<()> {
    // Hooks require a POSIX `sh` shell. Return a clear error on non-Unix
    // platforms rather than failing cryptically when `sh` is missing.
    #[cfg(not(unix))]
    {
        return Err(crate::error::CwError::Other(
            "hooks require 'sh' shell which is not available on Windows".into(),
        ));
    }

    // Worktrees are siblings of the main repo (default ../<repo>-<branch>),
    // so walking up from `cwd` would never find the main repo's .cwconfig.json.
    // Resolve to the main repo root for config lookup; `cwd` is kept as the
    // shell's working directory so hooks run inside the worktree they pertain to.
    let config_root =
        crate::git::get_main_repo_root(Some(cwd)).unwrap_or_else(|_| cwd.to_path_buf());
    let cfg = crate::config::load_effective_config(&config_root)?;
    let cmd = match event {
        "post_new" => cfg.hooks.post_new,
        "pre_rm" => cfg.hooks.pre_rm,
        _ => return Ok(()),
    };
    let Some(cmd) = cmd else {
        return Ok(());
    };
    // sh -c lets users write pipes/conditionals like "npm install && npm test".
    // stderr is inherited from Command::status() so users see hook output directly.
    let status = Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .current_dir(cwd)
        .status()?;
    if !status.success() {
        return Err(crate::error::CwError::Other(format!(
            "hook '{}' (`{}`) exited with {}",
            event,
            cmd.chars().take(60).collect::<String>(),
            status.code().unwrap_or(-1)
        )));
    }
    Ok(())
}
