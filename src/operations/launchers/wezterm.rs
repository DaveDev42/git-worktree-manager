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

/// Launch in new WezTerm tab without stealing focus.
///
/// Spawns a new tab, immediately restores focus to the original tab,
/// then sends the command to the new pane in the background.
pub fn launch_tab_bg(path: &Path, command: &str, ai_tool_name: &str) -> Result<()> {
    if !git::has_command("wezterm") {
        return Err(CwError::Git(
            "wezterm not installed. Install from https://wezterm.org/".to_string(),
        ));
    }

    // Find the currently active tab in the same window so we can restore focus.
    // We use WEZTERM_PANE to identify which window we belong to, then look for
    // the active tab in that window (which may differ from the calling tab if
    // the user has switched tabs since opening this shell).
    let current_pane = std::env::var("WEZTERM_PANE").unwrap_or_default();
    let original_tab_id = if !current_pane.is_empty() {
        get_active_tab_in_same_window(&current_pane)
    } else {
        None
    };

    let path_str = path.to_string_lossy().to_string();
    let output = Command::new("wezterm")
        .args(["cli", "spawn", "--cwd", &path_str])
        .output()
        .map_err(|e| CwError::Git(format!("wezterm spawn failed: {}", e)))?;

    let pane_id = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // Immediately restore focus to original tab before send_text polling
    if let Some(tab_id) = original_tab_id {
        let _ = Command::new("wezterm")
            .args(["cli", "activate-tab", "--tab-id", &tab_id])
            .status();
    } else {
        eprintln!(
            "{} WEZTERM_PANE not set; cannot restore focus to original tab",
            style("!").yellow()
        );
    }

    send_text(&pane_id, command)?;

    println!(
        "{} {} running in new WezTerm tab (background)\n",
        style("*").green().bold(),
        ai_tool_name
    );
    Ok(())
}

/// Get the tab_id of the currently active tab in the same window as `pane_id`.
///
/// This finds the window that `pane_id` belongs to, then returns the tab_id
/// of whichever tab is currently active in that window. This correctly handles
/// the case where the user has switched to a different tab after launching gw.
fn get_active_tab_in_same_window(pane_id: &str) -> Option<String> {
    let output = Command::new("wezterm")
        .args(["cli", "list", "--format", "json"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let panes: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).ok()?;
    find_active_tab_in_window(&panes, pane_id)
}

/// Pure function: find the active tab_id in the same window as `pane_id`.
///
/// Looks up which window owns `pane_id`, then finds the active pane in that
/// window and returns its tab_id.
fn find_active_tab_in_window(panes: &[serde_json::Value], pane_id: &str) -> Option<String> {
    let target: u64 = pane_id.parse().ok()?;

    // Find which window the calling pane belongs to
    let window_id = panes
        .iter()
        .find(|p| p["pane_id"].as_u64() == Some(target))
        .and_then(|p| p["window_id"].as_u64())?;

    // Find the active tab in that window.
    // Note: `is_active` is per-pane, not per-tab. WezTerm marks exactly one pane
    // per window as active (the focused pane in the active tab). Finding any
    // active pane in our window gives us the correct tab_id.
    panes
        .iter()
        .find(|p| {
            p["window_id"].as_u64() == Some(window_id) && p["is_active"].as_bool() == Some(true)
        })
        .and_then(|p| p["tab_id"].as_u64())
        .map(|t| t.to_string())
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_pane(window_id: u64, tab_id: u64, pane_id: u64, is_active: bool) -> serde_json::Value {
        json!({
            "window_id": window_id,
            "tab_id": tab_id,
            "pane_id": pane_id,
            "is_active": is_active,
        })
    }

    #[test]
    fn returns_active_tab_in_same_window() {
        let panes = vec![make_pane(1, 10, 100, false), make_pane(1, 11, 101, true)];
        assert_eq!(find_active_tab_in_window(&panes, "100"), Some("11".into()));
    }

    #[test]
    fn ignores_active_tab_in_different_window() {
        let panes = vec![make_pane(1, 10, 100, false), make_pane(2, 20, 200, true)];
        assert_eq!(find_active_tab_in_window(&panes, "100"), None);
    }

    #[test]
    fn returns_none_for_unknown_pane() {
        let panes = vec![make_pane(1, 10, 100, true)];
        assert_eq!(find_active_tab_in_window(&panes, "999"), None);
    }

    #[test]
    fn returns_none_for_invalid_pane_id() {
        let panes = vec![make_pane(1, 10, 100, true)];
        assert_eq!(find_active_tab_in_window(&panes, "not-a-number"), None);
    }

    #[test]
    fn handles_multi_pane_active_tab() {
        let panes = vec![make_pane(1, 10, 100, false), make_pane(1, 10, 101, true)];
        assert_eq!(find_active_tab_in_window(&panes, "100"), Some("10".into()));
    }

    #[test]
    fn returns_none_for_empty_pane_list() {
        let panes: Vec<serde_json::Value> = vec![];
        assert_eq!(find_active_tab_in_window(&panes, "100"), None);
    }

    #[test]
    fn returns_own_tab_when_caller_is_active() {
        // Caller's pane being active is a harmless no-op restore.
        let panes = vec![make_pane(1, 10, 100, true)];
        assert_eq!(find_active_tab_in_window(&panes, "100"), Some("10".into()));
    }
}
