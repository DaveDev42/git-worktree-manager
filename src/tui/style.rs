//! Shared color palette for ratatui-based views.
//!
//! Mirrors `src/console.rs::status_style` so the TUI and static renderers
//! produce visually identical output.

use ratatui::style::{Color, Modifier, Style};

pub fn status_style(status: &str) -> Style {
    match status {
        "clean" => Style::default().fg(Color::Green),
        "modified" => Style::default().fg(Color::Yellow),
        "busy" => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        "active" => Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
        "pr-open" => Style::default().fg(Color::Cyan),
        "merged" => Style::default().fg(Color::Magenta),
        "stale" => Style::default().fg(Color::DarkGray),
        _ => Style::default().add_modifier(Modifier::DIM),
    }
}

pub fn placeholder_style() -> Style {
    Style::default().add_modifier(Modifier::DIM)
}

pub fn header_style() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}
