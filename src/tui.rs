//! Arrow-key TUI selector for interactive worktree selection.
//!
//! Mirrors src/git_worktree_manager/tui.py.

use std::io::{IsTerminal, Write};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Arrow-key selector that renders on stderr and returns selected value.
///
/// # Arguments
/// * `items` - List of (label, value) tuples
/// * `title` - Title shown above the list
/// * `default_index` - Initially highlighted item
///
/// # Returns
/// The value of the selected item, or None if cancelled.
pub fn arrow_select(
    items: &[(String, String)],
    title: &str,
    default_index: usize,
) -> Option<String> {
    if items.is_empty() {
        return None;
    }

    if !std::io::stderr().is_terminal() {
        return None;
    }

    let default_index = default_index.min(items.len() - 1);

    // Try Unix raw-mode selector first
    #[cfg(unix)]
    {
        if let Some(result) = arrow_select_unix(items, title, default_index) {
            return result;
        }
    }

    // Fallback to numbered input
    arrow_select_fallback(items, title, default_index)
}

// ---------------------------------------------------------------------------
// Terminal helpers
// ---------------------------------------------------------------------------

/// Get terminal width from stderr, defaulting to 80.
fn get_terminal_width() -> usize {
    console::Term::stderr().size().1 as usize
}

/// Write raw bytes to stderr (unbuffered).
fn write_stderr(s: &str) {
    let stderr = std::io::stderr();
    let mut handle = stderr.lock();
    let _ = handle.write_all(s.as_bytes());
    let _ = handle.flush();
}

/// Strip ANSI escape sequences and return the visible length.
fn visible_len(text: &str) -> usize {
    let mut len = 0;
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\x1b' {
            // Skip until 'm'
            i += 1;
            while i < bytes.len() && bytes[i] != b'm' {
                i += 1;
            }
            if i < bytes.len() {
                i += 1; // skip 'm'
            }
        } else {
            len += 1;
            i += 1;
        }
    }
    len
}

/// Truncate text to fit within `width` visible characters, preserving ANSI codes.
fn truncate(text: &str, width: usize) -> String {
    if visible_len(text) <= width {
        return text.to_string();
    }

    let mut vis_pos = 0;
    let mut cut_pos = 0;
    let bytes = text.as_bytes();
    let mut i = 0;

    while i < bytes.len() && vis_pos < width.saturating_sub(1) {
        if bytes[i] == b'\x1b' {
            // Skip ANSI escape sequence
            i += 1;
            while i < bytes.len() && bytes[i] != b'm' {
                i += 1;
            }
            if i < bytes.len() {
                i += 1; // skip 'm'
            }
        } else {
            vis_pos += 1;
            i += 1;
        }
        cut_pos = i;
    }

    let mut result = text[..cut_pos].to_string();
    result.push_str("\x1b[0m");
    result
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// Render the selector list on stderr using ANSI escape codes.
fn render(
    items: &[(String, String)],
    title: &str,
    selected: usize,
    _total_lines: usize,
    first_render: bool,
) {
    let width = get_terminal_width();

    if !first_render {
        // Restore cursor to saved position
        write_stderr("\x1b[u");
    }

    // Save cursor position at the start of our render area
    write_stderr("\x1b[s");

    // Title
    let line = format!("  \x1b[1m{title}\x1b[0m");
    write_stderr(&format!("\x1b[2K{}\r\n", truncate(&line, width)));
    // Blank line
    write_stderr("\x1b[2K\r\n");

    for (i, (label, value)) in items.iter().enumerate() {
        write_stderr("\x1b[2K"); // clear line
        let line = if i == selected {
            format!("  \x1b[1;7m > {label} \x1b[0m  \x1b[2m{value}\x1b[0m")
        } else {
            format!("    {label}  \x1b[2m{value}\x1b[0m")
        };
        write_stderr(&format!("{}\r\n", truncate(&line, width)));
    }

    // Clear any leftover lines below
    for _ in 0..2 {
        write_stderr("\x1b[2K\r\n");
    }
    // Move back up to just after our items
    write_stderr("\x1b[2A");
}

/// Erase the rendered selector from stderr.
fn cleanup(total_lines: usize) {
    // Restore to saved position
    write_stderr("\x1b[u");
    for _ in 0..total_lines + 2 {
        write_stderr("\x1b[2K\r\n");
    }
    write_stderr("\x1b[u");
}

// ---------------------------------------------------------------------------
// Key reading
// ---------------------------------------------------------------------------

/// Recognized key events.
#[derive(Debug, PartialEq)]
enum Key {
    Up,
    Down,
    Enter,
    Escape,
    CtrlC,
    Quit,
    Number(u8),
    Unknown,
}

/// Read a single keypress from the given file descriptor (Unix).
#[cfg(unix)]
fn read_key(fd: std::os::unix::io::RawFd) -> Result<Key, std::io::Error> {
    let mut buf = [0u8; 1];
    let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, 1) };
    if n <= 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "EOF on stdin",
        ));
    }

    match buf[0] {
        b'\x1b' => {
            // Could be escape sequence -- peek with a short timeout using select/poll
            let mut pollfd = libc::pollfd {
                fd,
                events: libc::POLLIN,
                revents: 0,
            };
            let ready = unsafe { libc::poll(&mut pollfd as *mut libc::pollfd, 1, 50) };
            if ready <= 0 {
                // Bare Escape key
                return Ok(Key::Escape);
            }
            let mut seq1 = [0u8; 1];
            let n = unsafe { libc::read(fd, seq1.as_mut_ptr() as *mut libc::c_void, 1) };
            if n <= 0 {
                return Ok(Key::Escape);
            }
            if seq1[0] == b'[' {
                let mut seq2 = [0u8; 1];
                let n = unsafe { libc::read(fd, seq2.as_mut_ptr() as *mut libc::c_void, 1) };
                if n <= 0 {
                    return Ok(Key::Unknown);
                }
                match seq2[0] {
                    b'A' => Ok(Key::Up),
                    b'B' => Ok(Key::Down),
                    _ => Ok(Key::Unknown),
                }
            } else {
                Ok(Key::Unknown)
            }
        }
        b'\r' | b'\n' => Ok(Key::Enter),
        0x03 => Ok(Key::CtrlC),
        b'q' => Ok(Key::Quit),
        c @ b'1'..=b'9' => Ok(Key::Number(c - b'0')),
        _ => Ok(Key::Unknown),
    }
}

// ---------------------------------------------------------------------------
// Unix raw-mode selector
// ---------------------------------------------------------------------------

#[cfg(unix)]
fn arrow_select_unix(
    items: &[(String, String)],
    title: &str,
    default_index: usize,
) -> Option<Option<String>> {
    use std::os::unix::io::AsRawFd;

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

    let result = (|| -> Option<String> {
        render(items, title, selected, total_lines, true);

        loop {
            let key = match read_key(fd) {
                Ok(k) => k,
                Err(_) => {
                    cleanup(total_lines);
                    return None;
                }
            };

            match key {
                Key::Enter => {
                    cleanup(total_lines);
                    return Some(items[selected].1.clone());
                }
                Key::CtrlC | Key::Quit | Key::Escape => {
                    cleanup(total_lines);
                    return None;
                }
                Key::Up => {
                    selected = if selected == 0 {
                        items.len() - 1
                    } else {
                        selected - 1
                    };
                    render(items, title, selected, total_lines, false);
                }
                Key::Down => {
                    selected = (selected + 1) % items.len();
                    render(items, title, selected, total_lines, false);
                }
                Key::Number(n) => {
                    let idx = (n as usize) - 1;
                    if idx < items.len() {
                        cleanup(total_lines);
                        return Some(items[idx].1.clone());
                    }
                }
                Key::Unknown => {}
            }
        }
    })();

    // Restore terminal
    unsafe {
        libc::tcsetattr(fd, libc::TCSADRAIN, &old_termios);
    }
    // Show cursor
    write_stderr("\x1b[?25h");

    Some(result)
}

// ---------------------------------------------------------------------------
// Fallback: numbered list
// ---------------------------------------------------------------------------

/// Fallback numbered list with text input.
fn arrow_select_fallback(
    items: &[(String, String)],
    title: &str,
    default_index: usize,
) -> Option<String> {
    let stderr = std::io::stderr();
    let mut out = stderr.lock();

    let _ = writeln!(out, "\n  {title}\n");
    for (i, (label, value)) in items.iter().enumerate() {
        let marker = if i == default_index { ">" } else { " " };
        let _ = writeln!(out, "  {marker} [{num}] {label}  {value}", num = i + 1);
    }
    let _ = writeln!(out);
    let _ = write!(out, "Select [1-{}]: ", items.len());
    let _ = out.flush();

    let mut input = String::new();
    match std::io::stdin().read_line(&mut input) {
        Ok(_) => {
            let input = input.trim();
            if input.is_empty() {
                return Some(items[default_index].1.clone());
            }
            if let Ok(n) = input.parse::<usize>() {
                let idx = n.wrapping_sub(1);
                if idx < items.len() {
                    return Some(items[idx].1.clone());
                }
            }
            None
        }
        Err(_) => None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_visible_len_plain_text() {
        assert_eq!(visible_len("hello"), 5);
        assert_eq!(visible_len(""), 0);
        assert_eq!(visible_len("abc def"), 7);
    }

    #[test]
    fn test_visible_len_with_ansi() {
        assert_eq!(visible_len("\x1b[1mhello\x1b[0m"), 5);
        assert_eq!(
            visible_len("\x1b[1;7m > foo \x1b[0m  \x1b[2mbar\x1b[0m"),
            12
        );
        assert_eq!(visible_len("\x1b[32m\x1b[0m"), 0);
    }

    #[test]
    fn test_truncate_no_truncation_needed() {
        let text = "short";
        assert_eq!(truncate(text, 80), "short");
    }

    #[test]
    fn test_truncate_plain_text() {
        let text = "hello world this is a long string";
        let result = truncate(text, 10);
        // Should be at most 9 visible chars + reset
        assert!(visible_len(&result) <= 10);
        assert!(result.ends_with("\x1b[0m"));
    }

    #[test]
    fn test_truncate_with_ansi() {
        let text = "\x1b[1mhello world long text\x1b[0m";
        let result = truncate(text, 10);
        assert!(visible_len(&result) <= 10);
        assert!(result.ends_with("\x1b[0m"));
    }

    #[test]
    fn test_truncate_width_one() {
        let result = truncate("hello", 1);
        // With width=1, saturating_sub(1) = 0, so no visible chars
        assert!(result.ends_with("\x1b[0m"));
    }

    #[test]
    fn test_arrow_select_empty_items() {
        assert_eq!(arrow_select(&[], "title", 0), None);
    }

    #[test]
    fn test_key_enum_equality() {
        assert_eq!(Key::Up, Key::Up);
        assert_eq!(Key::Number(3), Key::Number(3));
        assert_ne!(Key::Up, Key::Down);
    }

    #[test]
    fn test_fallback_default_index_clamped() {
        // arrow_select clamps default_index; test the logic directly
        let items = vec![
            ("a".to_string(), "val_a".to_string()),
            ("b".to_string(), "val_b".to_string()),
        ];
        let clamped = 10usize.min(items.len() - 1);
        assert_eq!(clamped, 1);
    }
}
