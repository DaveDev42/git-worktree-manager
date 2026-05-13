//! TUI rendering layer built on ratatui + crossterm.
//!
//! Houses:
//! - `arrow_select`: raw-mode arrow-key single-select
//! - `multi_select`: raw-mode arrow + space multi-select (for `gw rm -i`)
//! - `raw_mode`:    RAII `RawModeGuard` for termios + cursor state (Unix-only)
//! - `list_view`:   Inline Viewport renderer for `gw list`
//! - `style`:       shared ratatui `Style` palette mirroring `crate::console`
//!
//! Simple commands with pure text output continue to use `crate::console`.
//! ratatui is reserved for commands that need declarative/progressive rendering.

pub mod arrow_select;
pub mod config_editor;
pub mod list_view;
pub mod multi_select;
pub mod raw_mode;
pub mod style;

// Re-export for backwards-compatible call sites that use `crate::tui::arrow_select(...)`.
pub use arrow_select::arrow_select;

use std::io::IsTerminal;
use std::sync::atomic::{AtomicBool, Ordering};

/// Whether stdout is attached to a terminal. Commands should fall back to
/// static rendering when this returns false (pipes, redirects, CI).
pub fn stdout_is_tty() -> bool {
    std::io::stdout().is_terminal()
}

// #20/#4: tracks whether a ratatui terminal is currently active. The panic hook
// checks this flag so `ratatui::restore()` is only called when it matters —
// a non-ratatui panic must not clobber terminal state it never set up.
//
// Single-thread invariant: only the main thread creates ratatui terminals
// in this codebase. Relaxed ordering is sufficient. If callers ever cross
// threads, upgrade to Acquire/Release.
static RATATUI_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Mark that a ratatui terminal is now active.
///
/// # Safety contract
/// Callers must pair every `mark_ratatui_active` with a `mark_ratatui_inactive`
/// on all exit paths (Ok / Err / panic). The canonical wrapper is
/// `display.rs::TerminalGuard`, which uses Drop to guarantee the pairing. The
/// fullscreen-alt-screen editor in `tui::config_editor` calls these directly
/// because its lifecycle differs (alt-screen + raw mode rather than inline
/// viewport), and it accepts the burden of manual cleanup on every arm,
/// including the `Terminal::new` failure path. New callers should prefer
/// `TerminalGuard` unless they have the same justification.
pub(crate) fn mark_ratatui_active() {
    RATATUI_ACTIVE.store(true, Ordering::Relaxed);
}

/// Mark that the ratatui terminal has been released.
///
/// # Safety contract
/// See [`mark_ratatui_active`]. Pair every `mark_ratatui_active` with exactly
/// one `mark_ratatui_inactive`, including on error and panic paths. Skipping
/// the clear leaves the panic hook armed to call `ratatui::restore()` on the
/// next unrelated panic, which can scribble on a healthy terminal.
pub(crate) fn mark_ratatui_inactive() {
    RATATUI_ACTIVE.store(false, Ordering::Relaxed);
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
        if RATATUI_ACTIVE.load(Ordering::Relaxed) {
            // #6: catch_unwind guards against a second panic inside restore().
            let _ = std::panic::catch_unwind(|| {
                ratatui::restore();
            });
        }
        default(info);
    }));
}
