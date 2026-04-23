# TUI Raw-Mode Guard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Introduce `RawModeGuard` so that a panic inside `arrow_select_unix` / `multi_select_unix` cannot leave the user's terminal in raw mode with the cursor hidden.

**Architecture:** New `#[cfg(unix)]` module `src/tui/raw_mode.rs` owning a small RAII type that saves termios on `enter()`, enters raw mode, optionally hides the cursor, and restores both on `Drop`. The two existing selectors drop their inline termios/cursor plumbing and replace it with a single `RawModeGuard::enter(fd, true)?` call at the top of each `*_unix` function. Line-clearing `cleanup(total_lines)` stays with the callers — separate concern from terminal mode.

**Tech Stack:** Rust, libc (`termios`, `tcgetattr`, `tcsetattr`, `cfmakeraw`), Unix-only.

---

## File Structure

- **Create:** `src/tui/raw_mode.rs` — `RawModeGuard` struct, `enter()`, `Drop`, and a tiny test module.
- **Modify:** `src/tui/mod.rs` — register the new module (`pub mod raw_mode;`).
- **Modify:** `src/tui/arrow_select.rs` — drop inline termios/cursor code in `arrow_select_unix`; use guard.
- **Modify:** `src/tui/multi_select.rs` — same treatment for `multi_select_unix`.

---

### Task 1: Scaffold `raw_mode.rs` with a failing test

**Files:**
- Create: `src/tui/raw_mode.rs`
- Modify: `src/tui/mod.rs`

- [ ] **Step 1: Create the module file with only the failing test**

```rust
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
    pub(crate) fn enter(_fd: i32, _hide_cursor: bool) -> Option<Self> {
        None // placeholder; implemented in Task 2
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        // placeholder; implemented in Task 2
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
```

- [ ] **Step 2: Register the module in `src/tui/mod.rs`**

Add `pub mod raw_mode;` alongside the other `pub mod` declarations (the module is `#![cfg(unix)]`-gated internally, so the outer declaration is unconditional — same pattern as other Unix-y helpers the crate already uses).

Exact edit to `src/tui/mod.rs`:
```rust
pub mod arrow_select;
pub mod list_view;
pub mod multi_select;
pub mod raw_mode;
pub mod style;
```

- [ ] **Step 3: Run the test to confirm it passes trivially**

Since `enter` currently returns `None`, the test passes even before real logic — that's fine. The placeholder makes the module compile; the real assertion (that `enter` returns `None` *because* `tcgetattr` failed, not because we hardcoded `None`) will hold after Task 2 when the stub is replaced with the real call.

Run:
```bash
cargo test -p git-worktree-manager raw_mode
```
Expected: 1 passing.

- [ ] **Step 4: Do NOT commit yet**

The placeholder `enter()` is not the real implementation. Commit at the end of Task 2 instead, so the first commit on the branch is a complete guard.

---

### Task 2: Implement `RawModeGuard::enter` and `Drop`

**Files:**
- Modify: `src/tui/raw_mode.rs`

- [ ] **Step 1: Replace the stub `enter` with the real implementation**

Replace the `impl RawModeGuard` block:

```rust
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
```

- [ ] **Step 2: Implement `Drop`**

Replace the stub:

```rust
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
```

- [ ] **Step 3: Re-run the bad-fd test**

Run:
```bash
cargo test -p git-worktree-manager raw_mode
```
Expected: 1 passing. Now the `None` return is a real tcgetattr failure, not a stub.

- [ ] **Step 4: Clippy on the new module**

Run:
```bash
cargo clippy --all-targets -- -D warnings
```
Expected: no warnings.

- [ ] **Step 5: Do NOT commit yet — continue to Task 3 and 4**

Commit once the callers are migrated; a partially-adopted guard is noise.

---

### Task 3: Refactor `arrow_select_unix` to use `RawModeGuard`

**Files:**
- Modify: `src/tui/arrow_select.rs`

- [ ] **Step 1: Replace the termios + cursor-hide preamble**

In `arrow_select_unix` (starts around line 257), the current preamble (lines 267–300 in pre-refactor file) sets up termios and cursor-hide manually. Replace it. Before:

```rust
    let stdin = std::io::stdin();
    let fd = stdin.as_raw_fd();

    // Save original terminal attributes
    let mut old_termios: libc::termios = unsafe { std::mem::zeroed() };
    if unsafe { libc::tcgetattr(fd, &mut old_termios) } != 0 {
        return None; // Can't get termios, fall back
    }

    let mut selected = default_index;
    let total_lines = items.len() + 2; // title + blank + items

    // Hide cursor
    write_stderr("\x1b[?25l");

    // Set raw mode
    let mut raw = old_termios;
    // cfmakeraw equivalent
    raw.c_iflag &= !(libc::IGNBRK
        | libc::BRKINT
        | libc::PARMRK
        | libc::ISTRIP
        | libc::INLCR
        | libc::IGNCR
        | libc::ICRNL
        | libc::IXON);
    raw.c_oflag &= !libc::OPOST;
    raw.c_lflag &= !(libc::ECHO | libc::ECHONL | libc::ICANON | libc::ISIG | libc::IEXTEN);
    raw.c_cflag &= !(libc::CSIZE | libc::PARENB);
    raw.c_cflag |= libc::CS8;
    raw.c_cc[libc::VMIN] = 1;
    raw.c_cc[libc::VTIME] = 0;

    if unsafe { libc::tcsetattr(fd, libc::TCSAFLUSH, &raw) } != 0 {
        write_stderr("\x1b[?25h");
        return None;
    }
```

After:

```rust
    let stdin = std::io::stdin();
    let fd = stdin.as_raw_fd();

    let _guard = super::raw_mode::RawModeGuard::enter(fd, true)?;

    let mut selected = default_index;
    let total_lines = items.len() + 2; // title + blank + items
```

Notes:
- The `enter(fd, true)?` returns `Option<Self>`, and `arrow_select_unix` itself returns `Option<Option<String>>`. The `?` here propagates `None` as "can't set up raw mode; caller falls back" — same behavior as the pre-refactor explicit `return None;` paths.
- `_guard` is bound with a leading underscore to signal "kept for `Drop`, not used by name."

- [ ] **Step 2: Remove the manual restore/show-cursor tail**

Before (around lines 347–352):

```rust
    // Restore terminal
    unsafe {
        libc::tcsetattr(fd, libc::TCSADRAIN, &old_termios);
    }
    // Show cursor
    write_stderr("\x1b[?25h");

    Some(result)
```

After:

```rust
    Some(result)
```

The guard's `Drop` runs when `_guard` goes out of scope at function end — on both the normal return path and on panic-unwind.

- [ ] **Step 3: Build and run existing arrow_select tests**

Run:
```bash
cargo test -p git-worktree-manager arrow_select
cargo build
```
Expected: all existing tests pass, build succeeds.

- [ ] **Step 4: Clippy**

Run:
```bash
cargo clippy --all-targets -- -D warnings
```
Expected: 0 warnings. Dead imports of `libc` items inside `arrow_select.rs` that were only used by the removed manual path should also be gone — if clippy complains about unused imports, remove them.

---

### Task 4: Refactor `multi_select_unix` to use `RawModeGuard`

**Files:**
- Modify: `src/tui/multi_select.rs`

- [ ] **Step 1: Replace the termios + cursor-hide preamble**

In `multi_select_unix` (around lines 34–54). Before:

```rust
    let stdin = std::io::stdin();
    let fd = stdin.as_raw_fd();

    // Save original terminal attributes
    let mut old_termios: libc::termios = unsafe { std::mem::zeroed() };
    if unsafe { libc::tcgetattr(fd, &mut old_termios) } != 0 {
        return None;
    }

    // Enter raw mode
    let mut raw = old_termios;
    unsafe { libc::cfmakeraw(&mut raw) };
    if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &raw) } != 0 {
        return None;
    }

    // Hide cursor
    write_stderr("\x1b[?25l");
```

After:

```rust
    let stdin = std::io::stdin();
    let fd = stdin.as_raw_fd();

    let _guard = super::raw_mode::RawModeGuard::enter(fd, true)?;
```

- [ ] **Step 2: Replace the cleanup tail**

Before (around lines 94–99):

```rust
    // Cleanup: show cursor, restore termios, clear our drawn lines
    write_stderr("\x1b[?25h");
    super::arrow_select::cleanup(total_lines);
    unsafe {
        libc::tcsetattr(fd, libc::TCSANOW, &old_termios);
    }

    Some(result)
```

After:

```rust
    // Clear our drawn lines on the happy path. Terminal mode + cursor are
    // handled by `_guard` going out of scope.
    super::arrow_select::cleanup(total_lines);

    Some(result)
```

- [ ] **Step 3: Drop the now-unused `use` of `write_stderr`**

The top-of-file `use super::arrow_select::{..., write_stderr, Key};` still needs `get_terminal_width`, `read_key`, `truncate`, `Key` — but `write_stderr` is now only used by `render`, so it stays. Double-check by running clippy in Step 5.

(If, after the edit, any of those imports become unused, clippy will flag them — remove whichever it names.)

- [ ] **Step 4: Build and run existing multi_select tests**

Run:
```bash
cargo test -p git-worktree-manager multi_select
cargo build
```
Expected: `empty_items_returns_empty_selection` passes; build succeeds.

- [ ] **Step 5: Clippy**

Run:
```bash
cargo clippy --all-targets -- -D warnings
```
Expected: 0 warnings.

---

### Task 5: Full verification

**Files:** none modified; verification only.

- [ ] **Step 1: fmt check**

```bash
cargo fmt --check
```
Expected: clean (exit 0).

- [ ] **Step 2: clippy all targets, warnings-as-errors**

```bash
cargo clippy --all-targets --all-features -- -D warnings
```
Expected: 0 warnings.

- [ ] **Step 3: full test suite**

```bash
cargo test --all-targets
```
Expected: all tests pass; count roughly matches the 460 tests (11 ignored) baseline called out in `CLAUDE.md`, with +1 for the new `enter_returns_none_on_bad_fd` test.

- [ ] **Step 4: Manual smoke (document only — do not run in CI)**

Script for the PR body:
- `cargo build && ./target/debug/gw _path -i` — select a worktree with Enter, then rerun and cancel with Esc. Terminal must be cooked and cursor visible both times.
- `./target/debug/gw delete -i` — reach the multi-select, exit via Enter with 0 selections, via Esc, via `q`, and via Ctrl-C. Each exit must leave a cooked terminal with the cursor visible.
- Optional panic-injection test (do NOT commit): temporarily add `panic!("inject")` after `RawModeGuard::enter` in one selector, run it, confirm that after the panic message prints, the shell prompt is usable (typing shows characters) and the cursor is visible. Remove the panic before committing.

---

### Task 6: Commit

**Files:** none modified; commit only.

- [ ] **Step 1: Stage the changes**

```bash
git add src/tui/raw_mode.rs src/tui/mod.rs src/tui/arrow_select.rs src/tui/multi_select.rs docs/superpowers/plans/2026-04-23-tui-raw-mode-guard.md
```

- [ ] **Step 2: Commit with the required conventional-commits prefix**

```bash
git commit -m "$(cat <<'EOF'
refactor(tui): RAII guard for raw-mode termios and cursor state

Introduce `RawModeGuard` in `src/tui/raw_mode.rs`. It owns the saved
termios and the cursor-hidden bit, so `Drop` restores both on every exit
path including panic-unwind. `arrow_select_unix` and `multi_select_unix`
replace their inline termios + cursor-hide plumbing with a single
`RawModeGuard::enter(fd, true)?` call.

Notes:
- `cfmakeraw` replaces `arrow_select`'s hand-rolled flag twiddling — the
  two are equivalent.
- `tcsetattr` uses `TCSADRAIN` (was `TCSAFLUSH` in arrow_select and
  `TCSANOW` in multi_select) — drain, no input discard.
- Line-clearing `cleanup(total_lines)` stays in the callers; on panic
  the drawn lines remain but the terminal is usable.

Not covered: Windows (selectors already fall through to cooked-mode
numbered fallback there).
EOF
)"
```

- [ ] **Step 3: Verify commit landed and tree is clean**

```bash
git status
git log --oneline -1
```
Expected: working tree clean; the top commit is the refactor above.
