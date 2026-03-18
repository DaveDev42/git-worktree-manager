/// Foreground launcher — run AI tool in current terminal.
use std::path::Path;
use std::process::Command;

/// Run command in the current terminal (blocking).
pub fn run(path: &Path, cmd: &str) {
    if cfg!(target_os = "windows") {
        let _ = Command::new("cmd")
            .args(["/C", cmd])
            .current_dir(path)
            .status();
    } else {
        let _ = Command::new("bash")
            .args(["-lc", cmd])
            .current_dir(path)
            .status();
    }
}
