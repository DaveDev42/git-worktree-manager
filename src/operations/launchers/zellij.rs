/// Zellij launchers.
use std::path::Path;
use std::process::Command;

use console::style;

use crate::error::{CwError, Result};
use crate::git;

/// Launch in new Zellij session.
pub fn launch_session(
    path: &Path,
    command: &str,
    ai_tool_name: &str,
    session_name: &str,
) -> Result<()> {
    if !git::has_command("zellij") {
        return Err(CwError::Git(
            "zellij not installed. Install from https://zellij.dev/".to_string(),
        ));
    }

    // Like the tab/pane variants below, the command is the session's only
    // program — keep a login shell after it exits so the session survives.
    // (tmux's session launcher doesn't need this: it `send-keys` into a
    // pre-spawned interactive shell rather than running the command as PID 1.)
    let wrapped = super::keep_shell_after(command);
    Command::new("zellij")
        .args(["-s", session_name, "--", "bash", "-lc", &wrapped])
        .current_dir(path)
        .status()
        .map_err(|e| CwError::Git(format!("zellij launch failed: {}", e)))?;

    println!(
        "{} {} ran in Zellij session '{}'\n",
        style("*").green().bold(),
        ai_tool_name,
        session_name
    );
    Ok(())
}

/// Launch in new Zellij tab.
pub fn launch_tab(path: &Path, command: &str, ai_tool_name: &str, tab_name: &str) -> Result<()> {
    if std::env::var("ZELLIJ").is_err() {
        return Err(CwError::Git(
            "--term zellij-tab requires running inside a Zellij session".to_string(),
        ));
    }

    let path_str = path.to_string_lossy().to_string();
    let wrapped = super::keep_shell_after(command);
    // `--close-on-exit` so the tab actually closes when the wrapped shell
    // exits. Without it Zellij holds the pane in an "exited" state and the
    // user has to press a key (often misread as Ctrl+C ignoring `exit`) to
    // clear it.
    Command::new("zellij")
        .args([
            "action",
            "new-tab",
            "--close-on-exit",
            "--name",
            tab_name,
            "--cwd",
            &path_str,
            "--",
            "bash",
            "-lc",
            &wrapped,
        ])
        .status()
        .map_err(|e| CwError::Git(format!("zellij new-tab failed: {}", e)))?;

    println!(
        "{} {} running in new Zellij tab '{}'\n",
        style("*").green().bold(),
        ai_tool_name,
        tab_name
    );
    Ok(())
}

/// Launch in Zellij split pane.
pub fn launch_pane(path: &Path, command: &str, ai_tool_name: &str, horizontal: bool) -> Result<()> {
    if std::env::var("ZELLIJ").is_err() {
        return Err(CwError::Git(
            "--term zellij-pane-* requires running inside a Zellij session".to_string(),
        ));
    }

    let direction = if horizontal { "right" } else { "down" };
    let path_str = path.to_string_lossy().to_string();
    let wrapped = super::keep_shell_after(command);
    // See `launch_tab` for why `--close-on-exit` matters: without it the pane
    // is held open in "exited" state after the wrapped shell ends.
    Command::new("zellij")
        .args([
            "action",
            "new-pane",
            "--close-on-exit",
            "-d",
            direction,
            "--cwd",
            &path_str,
            "--",
            "bash",
            "-lc",
            &wrapped,
        ])
        .status()
        .map_err(|e| CwError::Git(format!("zellij new-pane failed: {}", e)))?;

    let pane_type = if horizontal { "horizontal" } else { "vertical" };
    println!(
        "{} {} running in Zellij {} pane\n",
        style("*").green().bold(),
        ai_tool_name,
        pane_type
    );
    Ok(())
}
