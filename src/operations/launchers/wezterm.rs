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

    // Use CR, not LF: Windows PowerShell / PSReadLine treats CR as Enter and
    // ignores a bare LF. On Unix, the PTY's `icrnl` setting maps CR→LF before
    // the shell sees it, so bash/zsh accept CR too.
    let input_text = format!("{}\r", command);
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

    // Find the tab that owns $WEZTERM_PANE so we can restore focus to it.
    // Older code tried to find "whichever tab is active in this window" via
    // the JSON `is_active` field, but WezTerm on Windows reports `is_active`
    // per-pane-within-tab (every sole-pane tab is marked active), so the
    // search returned an arbitrary tab. Looking up the calling pane's tab
    // directly is reliable on every platform, at the cost of losing the
    // "user manually switched tabs between launching and spawn returning"
    // edge case — an acceptable trade for correct default behavior.
    let current_pane = std::env::var("WEZTERM_PANE").unwrap_or_default();
    let original_tab_id = if !current_pane.is_empty() {
        get_tab_for_pane(&current_pane)
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

/// Get the tab_id that owns `pane_id`, via `wezterm cli list --format json`.
fn get_tab_for_pane(pane_id: &str) -> Option<String> {
    let output = Command::new("wezterm")
        .args(["cli", "list", "--format", "json"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let panes: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).ok()?;
    find_tab_for_pane(&panes, pane_id)
}

/// Pure function: find the tab_id that owns `pane_id`.
fn find_tab_for_pane(panes: &[serde_json::Value], pane_id: &str) -> Option<String> {
    let target: u64 = pane_id.parse().ok()?;
    panes
        .iter()
        .find(|p| p["pane_id"].as_u64() == Some(target))
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

    fn make_pane(tab_id: u64, pane_id: u64) -> serde_json::Value {
        json!({
            "tab_id": tab_id,
            "pane_id": pane_id,
        })
    }

    #[test]
    fn returns_tab_for_matching_pane() {
        let panes = vec![make_pane(10, 100), make_pane(11, 101)];
        assert_eq!(find_tab_for_pane(&panes, "101"), Some("11".into()));
    }

    #[test]
    fn returns_none_for_unknown_pane() {
        let panes = vec![make_pane(10, 100)];
        assert_eq!(find_tab_for_pane(&panes, "999"), None);
    }

    #[test]
    fn returns_none_for_invalid_pane_id() {
        let panes = vec![make_pane(10, 100)];
        assert_eq!(find_tab_for_pane(&panes, "not-a-number"), None);
    }

    #[test]
    fn returns_none_for_empty_pane_list() {
        let panes: Vec<serde_json::Value> = vec![];
        assert_eq!(find_tab_for_pane(&panes, "100"), None);
    }

    #[test]
    fn ignores_is_active_flag() {
        // Windows WezTerm reports `is_active=true` on every sole-pane tab,
        // so the lookup must not rely on that field.
        let panes = vec![
            json!({"tab_id": 0, "pane_id": 0, "is_active": true}),
            json!({"tab_id": 9, "pane_id": 9, "is_active": true}),
        ];
        assert_eq!(find_tab_for_pane(&panes, "9"), Some("9".into()));
    }
}
