//! TUI rendering layer built on ratatui + crossterm.
//!
//! Houses:
//! - `arrow_select`: raw-mode arrow-key selector (pre-existing)
//! - `list_view`:   Inline Viewport renderer for `gw list` (new)
//! - `style`:       shared ratatui `Style` palette mirroring `crate::console`
//!
//! Simple commands with pure text output continue to use `crate::console`.
//! ratatui is reserved for commands that need declarative/progressive rendering.

pub mod arrow_select;
pub mod list_view;
pub mod style;

// Re-export the legacy selector so `crate::tui::arrow_select(...)` still works
// for existing callers that used the previous flat-file module shape.
pub use arrow_select::arrow_select;

use std::io::IsTerminal;

/// Whether stdout is attached to a terminal. Commands should fall back to
/// static rendering when this returns false (pipes, redirects, CI).
pub fn stdout_is_tty() -> bool {
    std::io::stdout().is_terminal()
}

/// Install a panic hook that restores the terminal state before the default
/// panic handler prints. Safe to call once at process start.
pub fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = ratatui::restore();
        default(info);
    }));
}
