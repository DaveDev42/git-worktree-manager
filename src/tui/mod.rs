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
use std::sync::atomic::{AtomicBool, Ordering};

/// Whether stdout is attached to a terminal. Commands should fall back to
/// static rendering when this returns false (pipes, redirects, CI).
pub fn stdout_is_tty() -> bool {
    std::io::stdout().is_terminal()
}

// #20: tracks whether a ratatui terminal is currently active. The panic hook
// checks this flag so `ratatui::restore()` is only called when it matters —
// a non-ratatui panic must not clobber terminal state it never set up.
static RATATUI_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Mark that a ratatui terminal is now active (call from `TerminalGuard::new`).
pub fn mark_ratatui_active() {
    RATATUI_ACTIVE.store(true, Ordering::SeqCst);
}

/// Mark that the ratatui terminal has been released (call from `TerminalGuard::drop`).
pub fn mark_ratatui_inactive() {
    RATATUI_ACTIVE.store(false, Ordering::SeqCst);
}

/// Install a panic hook that restores the terminal state before the default
/// panic handler prints. Safe to call once at process start.
///
/// The hook is gated on `RATATUI_ACTIVE` so it only calls `ratatui::restore()`
/// when a ratatui terminal is actually in use — avoiding spurious restores for
/// non-TTY panics (pipes, redirects, CI). `TerminalGuard` in `display.rs`
/// sets and clears this flag.
///
/// `default(info)` chains to the original hook, which prints the panic message
/// and respects `RUST_BACKTRACE` — so backtrace behaviour is preserved.
pub fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if RATATUI_ACTIVE.load(Ordering::SeqCst) {
            ratatui::restore();
        }
        default(info);
    }));
}
