//! RAII guard for Unix terminal raw-mode + cursor state.
//!
//! Callers that manipulate termios directly risk leaving the terminal in raw
//! mode with the cursor hidden if they panic mid-render. `RawModeGuard` owns
//! both pieces of state so `Drop` restores them on every exit path, including
//! panic-unwind.

#![cfg(unix)]

use std::io::Write;

pub(crate) struct RawModeGuard {
    fd: i32,
    original_termios: libc::termios,
    cursor_hidden: bool,
}

impl RawModeGuard {
    /// Enter raw mode on `fd`. If `hide_cursor` is true, also emits the
    /// hide-cursor escape sequence to stderr. Returns `None` if `tcgetattr`
    /// or `tcsetattr` fails — the caller falls back to cooked-mode I/O.
    pub(crate) fn enter(fd: i32, hide_cursor: bool) -> Option<Self> {
        let mut original_termios: libc::termios = unsafe { std::mem::zeroed() };
        if unsafe { libc::tcgetattr(fd, &mut original_termios) } != 0 {
            return None;
        }

        let mut raw = original_termios;
        unsafe { libc::cfmakeraw(&mut raw) };
        if unsafe { libc::tcsetattr(fd, libc::TCSADRAIN, &raw) } != 0 {
            return None;
        }

        // Hide cursor only after termios is in place, so a failed tcsetattr
        // never leaves a hidden cursor with no guard to restore it.
        if hide_cursor {
            let stderr = std::io::stderr();
            let mut handle = stderr.lock();
            let _ = handle.write_all(b"\x1b[?25l");
            let _ = handle.flush();
        }

        Some(Self {
            fd,
            original_termios,
            cursor_hidden: hide_cursor,
        })
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        if self.cursor_hidden {
            let stderr = std::io::stderr();
            let mut handle = stderr.lock();
            let _ = handle.write_all(b"\x1b[?25h");
            let _ = handle.flush();
        }
        // Errors are ignored: the process is already in trouble (likely mid-panic).
        unsafe { libc::tcsetattr(self.fd, libc::TCSADRAIN, &self.original_termios) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enter_returns_none_on_bad_fd() {
        // fd -1 is never a valid tty; tcgetattr must fail, enter must yield None.
        assert!(RawModeGuard::enter(-1, false).is_none());
        assert!(RawModeGuard::enter(-1, true).is_none());
    }
}
