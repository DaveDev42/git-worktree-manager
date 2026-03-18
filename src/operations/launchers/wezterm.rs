/// WezTerm launchers.
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use console::style;

use crate::config;
use crate::error::{CwError, Result};
use crate::git;

/// Wait for shell to be ready in a WezTerm pane.
fn wait_for_shell_ready(pane_id: &str, timeout: f64) {
    let poll_interval = Duration::from_millis(200);
    let deadline = Instant::now() + Duration::from_secs_f64(timeout);

    while Instant::now() < deadline {
        if let Ok(output) = Command::new("wezterm")
            .args(["cli", "get-text", "--pane-id", pane_id])
            .output()
        {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                if !text.trim().is_empty() {
                    return; // Shell is ready
                }
            }
        }
        thread::sleep(poll_interval);
    }
}

/// Send text to a WezTerm pane after waiting for readiness.
fn send_text(pane_id: &str, command: &str) -> Result<()> {
    if pane_id.is_empty() {
        return Err(CwError::Git(
            "Failed to get pane ID from WezTerm spawn".to_string(),
        ));
    }

    let timeout = config::load_config()
        .map(|c| c.launch.wezterm_ready_timeout)
        .unwrap_or(5.0);

    wait_for_shell_ready(pane_id, timeout);

    let input_text = format!("{}\n", command);
    let mut child = Command::new("wezterm")
        .args(["cli", "send-text", "--pane-id", pane_id, "--no-paste"])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| CwError::Git(format!("wezterm send-text failed: {}", e)))?;

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        let _ = stdin.write_all(input_text.as_bytes());
    }
    let _ = child.wait();

    Ok(())
}

/// Launch in new WezTerm window.
pub fn launch_window(path: &Path, command: &str, ai_tool_name: &str) -> Result<()> {
    if !git::has_command("wezterm") {
        return Err(CwError::Git(
            "wezterm not installed. Install from https://wezterm.org/".to_string(),
        ));
    }

    let path_str = path.to_string_lossy().to_string();
    let output = Command::new("wezterm")
        .args(["cli", "spawn", "--new-window", "--cwd", &path_str])
        .output()
        .map_err(|e| CwError::Git(format!("wezterm spawn failed: {}", e)))?;

    let pane_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
    send_text(&pane_id, command)?;

    println!(
        "{} {} running in new WezTerm window\n",
        style("*").green().bold(),
        ai_tool_name
    );
    Ok(())
}

/// Launch in new WezTerm tab.
pub fn launch_tab(path: &Path, command: &str, ai_tool_name: &str) -> Result<()> {
    if !git::has_command("wezterm") {
        return Err(CwError::Git(
            "wezterm not installed. Install from https://wezterm.org/".to_string(),
        ));
    }

    let path_str = path.to_string_lossy().to_string();
    let output = Command::new("wezterm")
        .args(["cli", "spawn", "--cwd", &path_str])
        .output()
        .map_err(|e| CwError::Git(format!("wezterm spawn failed: {}", e)))?;

    let pane_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
    send_text(&pane_id, command)?;

    println!(
        "{} {} running in new WezTerm tab\n",
        style("*").green().bold(),
        ai_tool_name
    );
    Ok(())
}

/// Launch in WezTerm split pane.
pub fn launch_pane(path: &Path, command: &str, ai_tool_name: &str, horizontal: bool) -> Result<()> {
    if !git::has_command("wezterm") {
        return Err(CwError::Git(
            "wezterm not installed. Install from https://wezterm.org/".to_string(),
        ));
    }

    let split_flag = if horizontal {
        "--horizontal"
    } else {
        "--bottom"
    };
    let path_str = path.to_string_lossy().to_string();
    let output = Command::new("wezterm")
        .args(["cli", "split-pane", split_flag, "--cwd", &path_str])
        .output()
        .map_err(|e| CwError::Git(format!("wezterm split-pane failed: {}", e)))?;

    let pane_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
    send_text(&pane_id, command)?;

    let pane_type = if horizontal { "horizontal" } else { "vertical" };
    println!(
        "{} {} running in WezTerm {} pane\n",
        style("*").green().bold(),
        ai_tool_name,
        pane_type
    );
    Ok(())
}
