//! Arrow/checkbox multi-select TUI used by `gw delete -i`.
//!
//! Built on the raw-mode plumbing shared from `arrow_select.rs`.

use std::io::IsTerminal;

#[cfg(unix)]
use super::arrow_select::{get_terminal_width, read_key, truncate, write_stderr, Key};

/// Multi-select entry point. Returns selected indices in ascending order,
/// or `None` if the user cancelled. An empty Vec means the user confirmed
/// with zero selections.
pub fn multi_select(items: &[String], title: &str) -> Option<Vec<usize>> {
    if items.is_empty() {
        return Some(Vec::new());
    }
    if !std::io::stderr().is_terminal() {
        return multi_select_fallback(items, title);
    }

    #[cfg(unix)]
    {
        if let Some(result) = multi_select_unix(items, title) {
            return result;
        }
    }

    multi_select_fallback(items, title)
}

// -- Unix raw-mode --------------------------------------------------------

#[cfg(unix)]
fn multi_select_unix(items: &[String], title: &str) -> Option<Option<Vec<usize>>> {
    use std::os::unix::io::AsRawFd;

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

    let mut cursor = 0usize;
    let mut checked: Vec<bool> = vec![false; items.len()];
    let total_lines = items.len() + 3; // title + blank + items + hint

    render(items, &checked, cursor, title, true);

    let result: Option<Vec<usize>> = loop {
        match read_key(fd) {
            Ok(Key::Up) => {
                cursor = cursor.saturating_sub(1);
                render(items, &checked, cursor, title, false);
            }
            Ok(Key::Down) => {
                if cursor + 1 < items.len() {
                    cursor += 1;
                }
                render(items, &checked, cursor, title, false);
            }
            Ok(Key::Space) => {
                checked[cursor] = !checked[cursor];
                render(items, &checked, cursor, title, false);
            }
            Ok(Key::Enter) => {
                break Some(
                    checked
                        .iter()
                        .enumerate()
                        .filter_map(|(i, &c)| if c { Some(i) } else { None })
                        .collect(),
                );
            }
            Ok(Key::Escape) | Ok(Key::Quit) | Ok(Key::CtrlC) | Err(_) => {
                break None;
            }
            _ => {}
        }
    };

    // Cleanup: show cursor, restore termios, clear our drawn lines
    write_stderr("\x1b[?25h");
    super::arrow_select::cleanup(total_lines);
    unsafe {
        libc::tcsetattr(fd, libc::TCSANOW, &old_termios);
    }

    Some(result)
}

#[cfg(unix)]
fn render(items: &[String], checked: &[bool], cursor: usize, title: &str, first: bool) {
    let width = get_terminal_width();

    if !first {
        write_stderr("\x1b[u");
    }
    write_stderr("\x1b[s");

    // Title
    let line = format!("  \x1b[1m{title}\x1b[0m");
    write_stderr(&format!("\x1b[2K{}\r\n", truncate(&line, width)));
    write_stderr("\x1b[2K\r\n");

    for (i, label) in items.iter().enumerate() {
        write_stderr("\x1b[2K");
        let mark = if checked[i] { "[x]" } else { "[ ]" };
        let line = if i == cursor {
            format!("  \x1b[1;7m > {mark} {label} \x1b[0m")
        } else {
            format!("    {mark} {label}")
        };
        write_stderr(&format!("{}\r\n", truncate(&line, width)));
    }

    // Hint line
    write_stderr("\x1b[2K");
    write_stderr("  \x1b[2m(Space: toggle, Enter: confirm, Esc/q: cancel)\x1b[0m\r\n");

    // Blank spacer
    write_stderr("\x1b[2K\r\n");
    // Move cursor back up above the trailing blank
    write_stderr("\x1b[2A");
}

// -- Fallback (non-Unix or non-TTY) --------------------------------------

fn multi_select_fallback(items: &[String], title: &str) -> Option<Vec<usize>> {
    eprintln!("{}", title);
    for (i, item) in items.iter().enumerate() {
        eprintln!("  [{}] {}", i + 1, item);
    }
    eprintln!("Enter numbers (space- or comma-separated), 'all', or blank to cancel:");
    let mut buf = String::new();
    if std::io::stdin().read_line(&mut buf).is_err() {
        return None;
    }
    let s = buf.trim();
    if s.is_empty() {
        return None;
    }
    if s.eq_ignore_ascii_case("all") {
        return Some((0..items.len()).collect());
    }
    let mut out = Vec::new();
    for part in s.split(|c: char| c == ',' || c.is_whitespace()) {
        if part.is_empty() {
            continue;
        }
        if let Ok(n) = part.parse::<usize>() {
            if n >= 1 && n <= items.len() {
                out.push(n - 1);
            }
        }
    }
    out.sort();
    out.dedup();
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_items_returns_empty_selection() {
        let out = multi_select(&[], "title");
        assert_eq!(out, Some(Vec::new()));
    }
}
