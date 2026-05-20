/// Foreground launcher — run AI tool in current terminal.
use std::path::Path;
use std::process::Command;

use console::style;

/// Run command in the current terminal (blocking).
///
/// Non-zero exit is surfaced as a yellow warning on stderr rather than
/// silently swallowed — the caller does not propagate errors from this
/// function because the foreground case is interactive (the user already
/// sees the failure in the terminal output), but we at least record that
/// something went wrong.
pub fn run(path: &Path, cmd: &str) {
    let status = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", cmd])
            .current_dir(path)
            .status()
    } else {
        Command::new("bash")
            .args(["-lc", cmd])
            .current_dir(path)
            .status()
    };

    match status {
        Ok(s) if !s.success() => {
            eprintln!(
                "{} command exited with non-zero status: {}",
                style("warning:").yellow(),
                s
            );
        }
        Err(e) => {
            eprintln!(
                "{} failed to launch command: {}",
                style("warning:").yellow(),
                e
            );
        }
        Ok(_) => {}
    }
}
