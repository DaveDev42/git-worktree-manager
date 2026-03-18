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

    Command::new("zellij")
        .args(["-s", session_name, "--", "bash", "-lc", command])
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
pub fn launch_tab(path: &Path, command: &str, ai_tool_name: &str) -> Result<()> {
    if std::env::var("ZELLIJ").is_err() {
        return Err(CwError::Git(
            "--term zellij-tab requires running inside a Zellij session".to_string(),
        ));
    }

    let path_str = path.to_string_lossy().to_string();
    Command::new("zellij")
        .args([
            "action", "new-tab", "--cwd", &path_str, "--", "bash", "-lc", command,
        ])
        .status()
        .map_err(|e| CwError::Git(format!("zellij new-tab failed: {}", e)))?;

    println!(
        "{} {} running in new Zellij tab\n",
        style("*").green().bold(),
        ai_tool_name
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
    Command::new("zellij")
        .args([
            "action", "new-pane", "-d", direction, "--cwd", &path_str, "--", "bash", "-lc", command,
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
