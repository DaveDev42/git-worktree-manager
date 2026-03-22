/// Console output formatting utilities.
///
/// Provides styled terminal output using the `console` crate.
use console::{style, Style, Term};

/// Print a styled header line.
pub fn print_header(text: &str) {
    let term = Term::stdout();
    let _ = term.write_line(&format!("{}", style(text).cyan().bold()));
}

/// Print a styled success message.
pub fn print_success(text: &str) {
    let _ = Term::stdout().write_line(&format!("{}", style(text).green()));
}

/// Print a styled warning message.
pub fn print_warning(text: &str) {
    let _ = Term::stderr().write_line(&format!("{}", style(text).yellow()));
}

/// Print a styled error message.
pub fn print_error(text: &str) {
    let _ = Term::stderr().write_line(&format!("{}", style(text).red()));
}

/// Print dimmed text.
pub fn print_dim(text: &str) {
    let _ = Term::stdout().write_line(&format!("{}", style(text).dim()));
}

/// Status → color mapping used by list, tree, global_ops.
pub fn status_style(status: &str) -> Style {
    match status {
        "active" => Style::new().green().bold(),
        "clean" => Style::new().green(),
        "modified" => Style::new().yellow(),
        "stale" => Style::new().red(),
        _ => Style::new(),
    }
}

/// Map worktree status string to a display icon.
pub fn status_icon(status: &str) -> &'static str {
    match status {
        "active" => "●",
        "clean" => "○",
        "modified" => "◉",
        "stale" => "x",
        _ => "○",
    }
}

/// Get current terminal width (fallback 80).
pub fn terminal_width() -> usize {
    Term::stdout().size().1 as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_icon_all_variants() {
        assert_eq!(status_icon("active"), "●");
        assert_eq!(status_icon("clean"), "○");
        assert_eq!(status_icon("modified"), "◉");
        assert_eq!(status_icon("stale"), "x");
    }

    #[test]
    fn test_status_icon_unknown_fallback() {
        assert_eq!(status_icon("unknown"), "○");
        assert_eq!(status_icon(""), "○");
        assert_eq!(status_icon("something_else"), "○");
    }

    #[test]
    fn test_status_style_known() {
        // Just ensure no panic for all known statuses
        let _ = status_style("active");
        let _ = status_style("clean");
        let _ = status_style("modified");
        let _ = status_style("stale");
        let _ = status_style("unknown");
    }
}
