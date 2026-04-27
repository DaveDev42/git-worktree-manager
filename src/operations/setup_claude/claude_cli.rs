//! Wrapper around the external `claude` CLI for plugin/marketplace ops.
//!
//! The trait exists so tests can inject a fake without spawning processes.
//! `RealClaudeCli` shells out via `std::process::Command`.

use std::path::Path;
use std::process::Command;

#[derive(Debug)]
pub enum ClaudeCliError {
    /// `claude` binary not on PATH.
    NotInstalled,
    /// `claude` exited with a non-zero status. Stderr captured for the user.
    NonZeroExit { code: Option<i32>, stderr: String },
    /// Failed to spawn the process for an OS-level reason.
    Io(std::io::Error),
}

impl std::fmt::Display for ClaudeCliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClaudeCliError::NotInstalled => write!(f, "`claude` CLI not found on PATH"),
            ClaudeCliError::NonZeroExit { code, stderr } => write!(
                f,
                "`claude` exited with status {:?}: {}",
                code,
                stderr.trim()
            ),
            ClaudeCliError::Io(e) => write!(f, "failed to invoke `claude`: {}", e),
        }
    }
}

impl std::error::Error for ClaudeCliError {}

pub trait ClaudeCli {
    fn is_available(&self) -> bool;
    fn marketplace_add(&self, path: &Path) -> Result<(), ClaudeCliError>;
    fn marketplace_update(&self, name: &str) -> Result<(), ClaudeCliError>;
    fn plugin_install(&self, slug: &str) -> Result<(), ClaudeCliError>;
    fn plugin_update(&self, slug: &str) -> Result<(), ClaudeCliError>;
}

pub struct RealClaudeCli;

impl RealClaudeCli {
    fn run(&self, args: &[&str]) -> Result<(), ClaudeCliError> {
        let out = Command::new("claude")
            .args(args)
            .output()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    ClaudeCliError::NotInstalled
                } else {
                    ClaudeCliError::Io(e)
                }
            })?;
        if !out.status.success() {
            return Err(ClaudeCliError::NonZeroExit {
                code: out.status.code(),
                stderr: String::from_utf8_lossy(&out.stderr).to_string(),
            });
        }
        Ok(())
    }
}

impl ClaudeCli for RealClaudeCli {
    fn is_available(&self) -> bool {
        Command::new("claude")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
    fn marketplace_add(&self, path: &Path) -> Result<(), ClaudeCliError> {
        // `--scope user` so the registration is global, not per-project.
        self.run(&[
            "plugin",
            "marketplace",
            "add",
            &path.display().to_string(),
            "--scope",
            "user",
        ])
    }
    fn marketplace_update(&self, name: &str) -> Result<(), ClaudeCliError> {
        self.run(&["plugin", "marketplace", "update", name])
    }
    fn plugin_install(&self, slug: &str) -> Result<(), ClaudeCliError> {
        self.run(&["plugin", "install", slug, "--scope", "user"])
    }
    fn plugin_update(&self, slug: &str) -> Result<(), ClaudeCliError> {
        self.run(&["plugin", "update", slug])
    }
}
