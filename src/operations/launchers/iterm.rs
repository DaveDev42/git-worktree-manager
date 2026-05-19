/// iTerm2 launchers (macOS only via AppleScript).
use std::path::Path;
use std::process::Command;

use console::style;

use crate::error::{CwError, Result};

fn ensure_macos() -> Result<()> {
    if cfg!(not(target_os = "macos")) {
        return Err(CwError::Git(
            "iTerm launchers only work on macOS".to_string(),
        ));
    }
    Ok(())
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Escape a string for safe inclusion inside an AppleScript double-quoted
/// literal: `"foo"` → `"\"foo\""`. Only `"` and `\` are special inside
/// AppleScript string literals — everything else is taken verbatim, including
/// shell metacharacters (the inner shell will see them after AppleScript's
/// `write text` re-emits them).
///
/// Required because the launcher commands we receive (e.g. `<self-exe>
/// _spawn-ai <path>` from `spawn_spec::materialize`) may legitimately contain
/// double-quoted segments when the binary or spec path contains spaces.
fn applescript_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
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
        applescript_escape(command),
    );
    let status = Command::new("bash")
        .args(["-lc", &script])
        .status()
        .map_err(|e| CwError::Git(format!("iTerm launch failed: {}", e)))?;
    if !status.success() {
        return Err(CwError::Git(format!(
            "iTerm launch failed (osascript exit {})",
            status
        )));
    }
    println!(
        "{} {} running in new iTerm window\n",
        style("*").green().bold(),
        ai_tool_name
    );
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
    set newTab to (create tab with default profile)
    tell current session of newTab
      write text "cd {} && {}"
    end tell
  end tell
end tell
APPLESCRIPT"#,
        shell_quote(&path.to_string_lossy()),
        applescript_escape(command),
    );
    let status = Command::new("bash")
        .args(["-lc", &script])
        .status()
        .map_err(|e| CwError::Git(format!("iTerm launch failed: {}", e)))?;
    if !status.success() {
        return Err(CwError::Git(format!(
            "iTerm launch failed (osascript exit {})",
            status
        )));
    }
    println!(
        "{} {} running in new iTerm tab\n",
        style("*").green().bold(),
        ai_tool_name
    );
    Ok(())
}

/// Launch in iTerm split pane.
pub fn launch_pane(path: &Path, command: &str, ai_tool_name: &str, horizontal: bool) -> Result<()> {
    ensure_macos()?;
    let direction = if horizontal {
        "horizontally"
    } else {
        "vertically"
    };
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
        cmd = applescript_escape(command),
    );
    let status = Command::new("bash")
        .args(["-lc", &script])
        .status()
        .map_err(|e| CwError::Git(format!("iTerm launch failed: {}", e)))?;
    if !status.success() {
        return Err(CwError::Git(format!(
            "iTerm launch failed (osascript exit {})",
            status
        )));
    }
    let pane_type = if horizontal { "horizontal" } else { "vertical" };
    println!(
        "{} {} running in iTerm {} pane\n",
        style("*").green().bold(),
        ai_tool_name,
        pane_type
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::applescript_escape;

    #[test]
    fn applescript_escape_passes_through_plain_text() {
        assert_eq!(
            applescript_escape("/usr/local/bin/gw _spawn-ai /tmp/x.json"),
            "/usr/local/bin/gw _spawn-ai /tmp/x.json"
        );
    }

    #[test]
    fn applescript_escape_quotes_double_quotes() {
        // Real-world trigger: spec path inside a directory with spaces gets
        // quoted by spawn_spec, producing `"…"` segments inside the line.
        assert_eq!(
            applescript_escape(r#""/My App/gw" _spawn-ai "/tmp/x y.json""#),
            r#"\"/My App/gw\" _spawn-ai \"/tmp/x y.json\""#
        );
    }

    #[test]
    fn applescript_escape_doubles_backslashes() {
        // Backslash must be escaped before quote to avoid double-encoding.
        assert_eq!(applescript_escape(r"a\b"), r"a\\b");
        assert_eq!(applescript_escape(r#"\""#), r#"\\\""#);
    }
}
