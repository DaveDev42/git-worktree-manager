//! Resolve the two mutually-exclusive `gw new` / `gw spawn` prompt sources
//! (`--prompt`, `--prompt-file`) into a single optional string.
//!
//! Mutual exclusion is enforced at parse time by `clap` (`ArgGroup` in `cli.rs`),
//! so this helper assumes at most one source is active.
//!
//! `--prompt -` is the stdin form: when the inline value is exactly `-`, the
//! prompt is read from stdin via the injected reader. This is the conventional
//! Unix idiom (heredoc / pipe input) and replaces the older `--prompt-stdin`
//! flag.

use std::path::Path;

use crate::error::{CwError, Result};

/// Sentinel value that means "read prompt from stdin" when passed as the
/// `--prompt` value.
pub const STDIN_SENTINEL: &str = "-";

/// Collapse the two prompt sources (`--prompt`, `--prompt-file`) into a
/// single optional string.
///
/// `--prompt -` reads from stdin. `stdin_reader` and `stdin_is_tty` are
/// injected so tests can drive both without touching real stdin. In
/// production `main` passes closures that wrap `std::io::stdin()` and
/// `IsTerminal::is_terminal()` respectively.
///
/// When `--prompt -` is used and stdin is a TTY, this errors instead of
/// blocking forever waiting for the user to type EOF — that's almost never
/// what was intended (typically the user forgot to pipe input or attach a
/// heredoc).
///
/// Trailing newline characters (`\r`, `\n`, or any mix thereof) are stripped
/// from the end of the resolved string — editors and heredocs routinely
/// append one or more, and the AI tool doesn't want them.
/// If the resolved string is empty or whitespace-only after stripping, `None`
/// is returned so downstream code behaves as if no prompt was given (avoids
/// passing an empty argv entry like `claude ""`).
pub fn resolve_prompt(
    inline: Option<String>,
    file: Option<&Path>,
    stdin_is_tty: impl FnOnce() -> bool,
    stdin_reader: impl FnOnce() -> std::io::Result<String>,
) -> Result<Option<String>> {
    let raw: Option<String> = if let Some(s) = inline {
        if s == STDIN_SENTINEL {
            // `--prompt -`: read from stdin. Reject TTY up-front so the
            // process doesn't hang forever waiting for an EOF the user
            // isn't going to send.
            if stdin_is_tty() {
                return Err(CwError::Other(
                    "--prompt - reads from standard input, but stdin is a terminal. \
                     Pipe content (e.g. `echo 'hi' | gw new feat-x --prompt -`) \
                     or use a heredoc."
                        .to_string(),
                ));
            }
            Some(
                stdin_reader()
                    .map_err(|e| CwError::Other(format!("failed to read --prompt -: {e}")))?,
            )
        } else {
            Some(s)
        }
    } else if let Some(p) = file {
        Some(std::fs::read_to_string(p).map_err(|e| {
            CwError::Other(format!(
                "failed to read --prompt-file '{}': {e}",
                p.display()
            ))
        })?)
    } else {
        None
    };

    Ok(raw.and_then(|s| {
        let trimmed = s.trim_end_matches(['\n', '\r']);
        if trimmed.trim().is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }))
}
