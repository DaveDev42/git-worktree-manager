//! Lifecycle hooks executed by gw at key worktree transitions.
//!
//! Configured via `hooks.post_new` / `hooks.pre_rm` keys in
//! `~/.config/git-worktree-manager/config.json` or `.cwconfig.json`.

use std::path::Path;
use std::process::Command;

use crate::error::Result;

/// Run the configured hook for `event` (one of `"post_new"`, `"pre_rm"`),
/// resolving the hook command from the layered config rooted at `cwd`.
///
/// No-op (returns `Ok(())`) when the event name is unknown or the hook is
/// unset. Hook is run as `sh -c <cmd>` with `cwd` as the current directory.
/// A non-zero exit propagates as `CwError::Other`.
pub fn run_event(event: &str, cwd: &Path) -> Result<()> {
    let cfg = crate::config::load_effective_config(cwd)?;
    let cmd = match event {
        "post_new" => cfg.hooks.post_new,
        "pre_rm" => cfg.hooks.pre_rm,
        _ => return Ok(()),
    };
    let Some(cmd) = cmd else {
        return Ok(());
    };
    let status = Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .current_dir(cwd)
        .status()?;
    if !status.success() {
        return Err(crate::error::CwError::Other(format!(
            "hook '{}' exited with {}",
            event,
            status.code().unwrap_or(-1)
        )));
    }
    Ok(())
}
