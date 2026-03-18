/// tmux launchers.
use std::path::Path;
use std::process::Command;

use console::style;

use crate::error::{CwError, Result};
use crate::git;

/// Launch in new tmux session.
pub fn launch_session(
    path: &Path,
    command: &str,
    ai_tool_name: &str,
    session_name: &str,
) -> Result<()> {
    if !git::has_command("tmux") {
        return Err(CwError::Git(
            "tmux not installed. Install from https://tmux.github.io/".to_string(),
        ));
    }

    let path_str = path.to_string_lossy().to_string();

    // Create detached session
    Command::new("tmux")
        .args(["new-session", "-d", "-s", session_name, "-c", &path_str])
        .status()
        .map_err(|e| CwError::Git(format!("tmux new-session failed: {}", e)))?;

    // Send command
    Command::new("tmux")
        .args(["send-keys", "-t", session_name, command, "Enter"])
        .status()
        .map_err(|e| CwError::Git(format!("tmux send-keys failed: {}", e)))?;

    // Attach
    Command::new("tmux")
        .args(["attach-session", "-t", session_name])
        .status()
        .map_err(|e| CwError::Git(format!("tmux attach failed: {}", e)))?;

    println!(
        "{} {} ran in tmux session '{}'\n",
        style("*").green().bold(),
        ai_tool_name,
        session_name
    );
    Ok(())
}

/// Launch in new tmux window (requires active tmux session).
pub fn launch_window(path: &Path, command: &str, ai_tool_name: &str) -> Result<()> {
    if std::env::var("TMUX").is_err() {
        return Err(CwError::Git(
            "--term tmux-window requires running inside a tmux session".to_string(),
        ));
    }

    let path_str = path.to_string_lossy().to_string();
    Command::new("tmux")
        .args(["new-window", "-c", &path_str, "bash", "-lc", command])
        .status()
        .map_err(|e| CwError::Git(format!("tmux new-window failed: {}", e)))?;

    println!("{} {} running in new tmux window\n", style("*").green().bold(), ai_tool_name);
    Ok(())
}

/// Launch in tmux split pane.
pub fn launch_pane(
    path: &Path,
    command: &str,
    ai_tool_name: &str,
    horizontal: bool,
) -> Result<()> {
    if std::env::var("TMUX").is_err() {
        return Err(CwError::Git(
            "--term tmux-pane-* requires running inside a tmux session".to_string(),
        ));
    }

    let split_flag = if horizontal { "-h" } else { "-v" };
    let path_str = path.to_string_lossy().to_string();
    Command::new("tmux")
        .args([
            "split-window",
            split_flag,
            "-c",
            &path_str,
            "bash",
            "-lc",
            command,
        ])
        .status()
        .map_err(|e| CwError::Git(format!("tmux split-window failed: {}", e)))?;

    let pane_type = if horizontal { "horizontal" } else { "vertical" };
    println!(
        "{} {} running in tmux {} pane\n",
        style("*").green().bold(),
        ai_tool_name,
        pane_type
    );
    Ok(())
}
