/// WezTerm launchers.
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use console::style;

use crate::config;
use crate::error::{CwError, Result};
use crate::git;

/// Heuristic: does the last non-blank line of pane text look like an
/// interactive shell prompt waiting for input? We accept the common shell
/// prompt suffixes (`$`, `#`, `%`, `>`) and the prompt characters used by
/// popular zsh themes (`❯`, `➜`, `»`). The line is allowed to have trailing
/// whitespace (most prompts end with `<char> ` so the cursor sits one cell
/// past the marker).
fn looks_like_prompt(text: &str) -> bool {
    let last = text
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("");
    let trimmed = last.trim_end();
    matches!(
        trimmed.chars().last(),
        Some('$') | Some('#') | Some('%') | Some('>') | Some('❯') | Some('➜') | Some('»')
    )
}

/// Wait for the shell in a WezTerm pane to start reading stdin.
///
/// `wezterm cli get-text` returns whatever is currently rendered in the
/// pane, which under a multiplexer (zellij/tmux auto-attach via shell rc)
/// can be non-empty *before* any shell has actually started — the
/// multiplexer's UI chrome is enough to make a "non-empty" check fire
/// instantly. Sending text in that window gets dropped.
///
/// We wait until the last non-blank line ends in a prompt character.
/// To avoid flaky pre-prompt false positives (e.g. a banner line that
/// happens to end in `>`), we enforce a minimum settle delay before
/// polling starts.
fn wait_for_shell_ready(pane_id: &str, timeout: f64) {
    // Hard minimum: even on the fastest shells, wezterm's pane needs a few
    // hundred ms to render a prompt. Polling sooner just burns CPU.
    let min_delay = Duration::from_millis(400);
    thread::sleep(min_delay);

    let poll_interval = Duration::from_millis(150);
    let deadline = Instant::now() + Duration::from_secs_f64(timeout);

    while Instant::now() < deadline {
        if let Ok(output) = Command::new("wezterm")
            .args(["cli", "get-text", "--pane-id", pane_id])
            .output()
        {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                if looks_like_prompt(&text) {
                    return;
                }
            }
        }
        thread::sleep(poll_interval);
    }
}

/// Set the tab title for the tab containing `pane_id`. Best-effort: a
/// failure here is cosmetic, so we swallow errors rather than abort the launch.
fn set_tab_title(pane_id: &str, title: &str) {
    if pane_id.is_empty() || title.is_empty() {
        return;
    }
    let _ = Command::new("wezterm")
        .args(["cli", "set-tab-title", "--pane-id", pane_id, title])
        .status();
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
pub fn launch_window(
    path: &Path,
    command: &str,
    ai_tool_name: &str,
    tab_title: &str,
) -> Result<()> {
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
    set_tab_title(&pane_id, tab_title);
    send_text(&pane_id, command)?;

    println!(
        "{} {} running in new WezTerm window '{}'\n",
        style("*").green().bold(),
        ai_tool_name,
        tab_title
    );
    Ok(())
}

/// Launch in new WezTerm tab.
pub fn launch_tab(path: &Path, command: &str, ai_tool_name: &str, tab_title: &str) -> Result<()> {
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
    set_tab_title(&pane_id, tab_title);
    send_text(&pane_id, command)?;

    println!(
        "{} {} running in new WezTerm tab '{}'\n",
        style("*").green().bold(),
        ai_tool_name,
        tab_title
    );
    Ok(())
}

/// Launch in new WezTerm tab without stealing focus.
///
/// Spawns a new tab, immediately restores focus to the original tab,
/// then sends the command to the new pane in the background.
pub fn launch_tab_bg(
    path: &Path,
    command: &str,
    ai_tool_name: &str,
    tab_title: &str,
) -> Result<()> {
    if !git::has_command("wezterm") {
        return Err(CwError::Git(
            "wezterm not installed. Install from https://wezterm.org/".to_string(),
        ));
    }

    // Capture $WEZTERM_PANE up-front so we can restore focus after spawning.
    // `wezterm cli activate-pane --pane-id <id>` activates the pane directly
    // (and, by extension, its tab) without any JSON lookup — cross-platform
    // reliable where the old `is_active`-based tab search was not.
    let current_pane = std::env::var("WEZTERM_PANE").ok().filter(|s| !s.is_empty());

    let path_str = path.to_string_lossy().to_string();
    let output = Command::new("wezterm")
        .args(["cli", "spawn", "--cwd", &path_str])
        .output()
        .map_err(|e| CwError::Git(format!("wezterm spawn failed: {}", e)))?;

    let pane_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
    set_tab_title(&pane_id, tab_title);

    // Immediately restore focus to the original pane before send_text polling.
    if let Some(pane) = &current_pane {
        let _ = Command::new("wezterm")
            .args(["cli", "activate-pane", "--pane-id", pane])
            .status();
    } else {
        eprintln!(
            "{} WEZTERM_PANE not set; cannot restore focus",
            style("!").yellow()
        );
    }

    send_text(&pane_id, command)?;

    println!(
        "{} {} running in new WezTerm tab '{}' (background)\n",
        style("*").green().bold(),
        ai_tool_name,
        tab_title
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

#[cfg(test)]
mod tests {
    use super::looks_like_prompt;

    #[test]
    fn detects_common_prompt_suffixes() {
        for line in [
            "user@host:~ $",
            "user@host:~ $ ",
            "user@host:~ #",
            "/tmp %",
            "PS C:\\>",
            "~/code ❯",
            "myproj ➜",
            "» ",
        ] {
            assert!(looks_like_prompt(line), "expected prompt: {:?}", line);
        }
    }

    #[test]
    fn rejects_multiplexer_chrome_without_prompt() {
        // The exact failure mode this guards against: zellij's status bar is
        // non-empty and ends with whitespace before the shell has started.
        let chrome = " Zellij (a947) NORMAL  Tab #1                              \n";
        assert!(!looks_like_prompt(chrome));
    }

    #[test]
    fn rejects_blank_pane() {
        assert!(!looks_like_prompt(""));
        assert!(!looks_like_prompt("\n\n\n   \n"));
    }

    #[test]
    fn picks_last_non_blank_line() {
        // A prompt scrolled up by a banner is not ready — the last line is
        // blank, so we keep waiting.
        let text = "welcome banner\n$\n\n   \n";
        assert!(looks_like_prompt(text));
    }

    #[test]
    fn ignores_command_in_progress() {
        // Text that ends with a running command (no prompt char) should not
        // be treated as ready.
        let text = "user@host:~ $ echo hi\nhi\n";
        // Last non-blank line is "hi" — no prompt suffix.
        assert!(!looks_like_prompt(text));
    }
}
