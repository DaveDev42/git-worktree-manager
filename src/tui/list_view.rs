//! `gw list` Inline Viewport view.

use ratatui::layout::Constraint;
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Cell, Row, Table};

use crate::tui::style;

#[derive(Debug, Clone)]
pub struct RowData {
    pub worktree_id: String,
    pub current_branch: String,
    pub status: String, // "…" while pending
    pub age: String,
    pub rel_path: String,
}

pub struct ListApp {
    pub rows: Vec<RowData>,
}

impl ListApp {
    pub fn new(rows: Vec<RowData>) -> Self {
        Self { rows }
    }

    pub fn is_complete(&self) -> bool {
        self.rows.iter().all(|r| r.status != "…")
    }

    pub fn render(&self, frame: &mut ratatui::Frame<'_>) {
        let header = Row::new(vec![
            Cell::from("WORKTREE"),
            Cell::from("BRANCH"),
            Cell::from("STATUS"),
            Cell::from("AGE"),
            Cell::from("PATH"),
        ])
        .style(style::header_style());

        let body: Vec<Row> = self
            .rows
            .iter()
            .map(|r| {
                let status_cell = if r.status == "…" {
                    Cell::from(Span::styled("…", style::placeholder_style()))
                } else {
                    Cell::from(Span::styled(r.status.clone(), style::status_style(&r.status)))
                };
                Row::new(vec![
                    Cell::from(r.worktree_id.clone()),
                    Cell::from(r.current_branch.clone()),
                    status_cell,
                    Cell::from(r.age.clone()),
                    Cell::from(r.rel_path.clone()),
                ])
            })
            .collect();

        let widths = [
            Constraint::Percentage(20),
            Constraint::Percentage(25),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Percentage(35),
        ];

        let table = Table::new(body, widths)
            .header(header)
            .block(Block::default().borders(Borders::NONE));

        frame.render_widget(table, frame.area());
    }
}

use std::sync::mpsc;

/// Drive the Inline Viewport render loop, consuming `(row_index, status)`
/// updates from `rx` until all rows are filled or the sender disconnects.
///
/// The caller is responsible for spawning the producer (typically a
/// `rayon::spawn` that iterates worktrees in parallel and sends results).
///
/// On return, `app.rows` contains final statuses. The viewport exits via
/// `drop(terminal)` which leaves the final frame in the scrollback.
pub fn run<B: ratatui::backend::Backend>(
    terminal: &mut ratatui::Terminal<B>,
    app: &mut ListApp,
    rx: mpsc::Receiver<(usize, String)>,
) -> std::io::Result<()> {
    terminal.draw(|f| app.render(f))?;

    loop {
        match rx.recv_timeout(std::time::Duration::from_millis(50)) {
            Ok((i, status)) => {
                if let Some(r) = app.rows.get_mut(i) {
                    r.status = status;
                }
                terminal.draw(|f| app.render(f))?;
                if app.is_complete() {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if app.is_complete() {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn sample_row(id: &str, status: &str) -> RowData {
        RowData {
            worktree_id: id.to_string(),
            current_branch: id.to_string(),
            status: status.to_string(),
            age: "1d ago".to_string(),
            rel_path: format!("wt/{}", id),
        }
    }

    #[test]
    fn skeleton_frame_shows_ellipsis_for_all_rows() {
        let app = ListApp::new(vec![
            sample_row("feat/a", "…"),
            sample_row("feat/b", "…"),
        ]);
        let backend = TestBackend::new(80, 6);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| app.render(f)).unwrap();
        let buf = terminal.backend().buffer().clone();
        let rendered = buffer_to_string(&buf);
        assert!(rendered.contains("feat/a"));
        assert!(rendered.contains("feat/b"));
        assert!(rendered.contains("…"));
        assert!(!app.is_complete());
    }

    #[test]
    fn complete_frame_shows_final_status() {
        let app = ListApp::new(vec![
            sample_row("feat/a", "clean"),
            sample_row("feat/b", "modified"),
        ]);
        let backend = TestBackend::new(80, 6);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| app.render(f)).unwrap();
        let buf = terminal.backend().buffer().clone();
        let rendered = buffer_to_string(&buf);
        assert!(rendered.contains("clean"));
        assert!(rendered.contains("modified"));
        assert!(app.is_complete());
    }

    #[test]
    fn run_fills_statuses_from_channel() {
        let mut app = ListApp::new(vec![
            sample_row("feat/a", "…"),
            sample_row("feat/b", "…"),
        ]);
        let backend = TestBackend::new(80, 6);
        let mut terminal = Terminal::new(backend).unwrap();

        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            tx.send((0, "clean".to_string())).unwrap();
            tx.send((1, "modified".to_string())).unwrap();
        });

        run(&mut terminal, &mut app, rx).unwrap();
        assert_eq!(app.rows[0].status, "clean");
        assert_eq!(app.rows[1].status, "modified");
        assert!(app.is_complete());
    }

    fn buffer_to_string(buf: &ratatui::buffer::Buffer) -> String {
        let mut out = String::new();
        let area = buf.area();
        for y in 0..area.height {
            for x in 0..area.width {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }
}
