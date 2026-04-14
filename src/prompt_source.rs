//! Resolve the three mutually-exclusive `gw new` prompt sources
//! (`--prompt`, `--prompt-file`, `--prompt-stdin`) into a single optional string.
//!
//! Mutual exclusion is enforced at parse time by `clap` (`ArgGroup` in `cli.rs`),
//! so this helper assumes at most one source is active.

use std::path::Path;

use crate::error::{CwError, Result};

/// Collapse the three prompt sources into a single optional string.
///
/// `stdin_reader` is injected so tests can drive the stdin path without touching
/// the real stdin. In production `main` passes a closure that reads from `std::io::stdin()`.
///
/// A single trailing `\n` (and optional `\r`) is stripped from the resolved
/// string — most editors and heredocs append one, and the AI tool doesn't want it.
/// If the resolved string is empty or whitespace-only after stripping, `None` is
/// returned so downstream code behaves as if no prompt was given (avoids passing
/// an empty argv entry like `claude ""`).
pub fn resolve_prompt(
    inline: Option<String>,
    file: Option<&Path>,
    stdin: bool,
    stdin_reader: impl FnOnce() -> std::io::Result<String>,
) -> Result<Option<String>> {
    let raw: Option<String> = if let Some(s) = inline {
        Some(s)
    } else if let Some(p) = file {
        Some(std::fs::read_to_string(p).map_err(|e| {
            CwError::Other(format!(
                "failed to read --prompt-file '{}': {e}",
                p.display()
            ))
        })?)
    } else if stdin {
        Some(
            stdin_reader()
                .map_err(|e| CwError::Other(format!("failed to read --prompt-stdin: {e}")))?,
        )
    } else {
        None
    };

    Ok(raw.and_then(|s| {
        let trimmed = s.strip_suffix('\n').unwrap_or(&s);
        let trimmed = trimmed.strip_suffix('\r').unwrap_or(trimmed);
        if trimmed.trim().is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }))
}
