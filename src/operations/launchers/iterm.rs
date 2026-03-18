/// iTerm2 launchers (macOS only via AppleScript).
use std::path::Path;
use std::process::Command;

use console::style;

use crate::error::{CwError, Result};

fn ensure_macos() -> Result<()> {
    if cfg!(not(target_os = "macos")) {
        return Err(CwError::Git("iTerm launchers only work on macOS".to_string()));
    }
    Ok(())
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Launch in new iTerm window.
pub fn launch_window(path: &Path, command: &str, ai_tool_name: &str) -> Result<()> {
    ensure_macos()?;
    let script = format!(
        r#"osascript <<'APPLESCRIPT'
tell application "iTerm"
  activate
  set newWindow to (create window with default profile)
  tell current session of newWindow
    write text "cd {} && {}"
  end tell
end tell
APPLESCRIPT"#,
        shell_quote(&path.to_string_lossy()),
        command
    );
    Command::new("bash")
        .args(["-lc", &script])
        .status()
        .map_err(|e| CwError::Git(format!("iTerm launch failed: {}", e)))?;
    println!("{} {} running in new iTerm window\n", style("*").green().bold(), ai_tool_name);
    Ok(())
}

/// Launch in new iTerm tab.
pub fn launch_tab(path: &Path, command: &str, ai_tool_name: &str) -> Result<()> {
    ensure_macos()?;
    let script = format!(
        r#"osascript <<'APPLESCRIPT'
tell application "iTerm"
  activate
  tell current window
    create tab with default profile
    tell current session
      write text "cd {} && {}"
    end tell
  end tell
end tell
APPLESCRIPT"#,
        shell_quote(&path.to_string_lossy()),
        command
    );
    Command::new("bash")
        .args(["-lc", &script])
        .status()
        .map_err(|e| CwError::Git(format!("iTerm launch failed: {}", e)))?;
    println!("{} {} running in new iTerm tab\n", style("*").green().bold(), ai_tool_name);
    Ok(())
}

/// Launch in iTerm split pane.
pub fn launch_pane(path: &Path, command: &str, ai_tool_name: &str, horizontal: bool) -> Result<()> {
    ensure_macos()?;
    let direction = if horizontal { "horizontally" } else { "vertically" };
    let script = format!(
        r#"osascript <<'APPLESCRIPT'
tell application "iTerm"
  activate
  tell current session of current window
    split {direction} with default profile
  end tell
  tell last session of current tab of current window
    write text "cd {path} && {cmd}"
  end tell
end tell
APPLESCRIPT"#,
        direction = direction,
        path = shell_quote(&path.to_string_lossy()),
        cmd = command,
    );
    Command::new("bash")
        .args(["-lc", &script])
        .status()
        .map_err(|e| CwError::Git(format!("iTerm launch failed: {}", e)))?;
    let pane_type = if horizontal { "horizontal" } else { "vertical" };
    println!("{} {} running in iTerm {} pane\n", style("*").green().bold(), ai_tool_name, pane_type);
    Ok(())
}
