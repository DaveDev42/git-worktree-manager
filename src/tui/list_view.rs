//! `gw list` Inline Viewport view.

use std::sync::mpsc;

use ratatui::layout::Constraint;
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Cell, Row, Table};

use crate::tui::style;

/// Three ASCII dots; portable across terminals lacking Unicode rendering.
/// Do not change to U+2026 ('…'); equality checks elsewhere depend on this exact value.
pub const PLACEHOLDER: &str = "...";

#[derive(Debug, Clone)]
pub struct RowData {
    pub worktree_id: String,
    pub current_branch: String,
    // `status` uses owned String for simplicity; Cow<'static, str> would avoid
    // the placeholder allocation but adds lifetime gymnastics. Deferred.
    pub status: String, // PLACEHOLDER while pending
    pub age: String,
    pub rel_path: String,
}

pub struct ListApp {
    rows: Vec<RowData>,
}

impl ListApp {
    pub fn new(rows: Vec<RowData>) -> Self {
        Self { rows }
    }

    /// Read-only access to rows.
    pub fn rows(&self) -> &[RowData] {
        &self.rows
    }

    /// Update a row's status. Rejects PLACEHOLDER (use `finalize_pending` to bulk-reset).
    /// In debug builds, calling with PLACEHOLDER trips a debug_assert.
    pub(crate) fn set_status(&mut self, i: usize, status: String) {
        debug_assert_ne!(
            status, PLACEHOLDER,
            "set_status should never restore the placeholder"
        );
        if let Some(r) = self.rows.get_mut(i) {
            r.status = status;
        }
    }

    /// Replace every row whose status equals `PLACEHOLDER` with `replacement`.
    /// Called after the producer finishes (or panics) to ensure no placeholder
    /// remains in the final output.
    ///
    /// After calling `finalize_pending`, all rows have non-PLACEHOLDER status;
    /// `is_complete()` will then always return true. Calling `set_status` after
    /// `finalize_pending` is undefined.
    ///
    /// Returns `true` if any rows were updated (i.e., had PLACEHOLDER status),
    /// `false` if nothing changed — lets callers skip a redundant redraw.
    #[must_use = "ignoring whether any rows changed may cause redundant or missing redraws"]
    pub fn finalize_pending(&mut self, replacement: &str) -> bool {
        let mut changed = false;
        for r in self.rows.iter_mut() {
            if r.status == PLACEHOLDER {
                r.status = replacement.to_string();
                changed = true;
            }
        }
        changed
    }

    /// Consume `self`, yielding the inner rows.
    pub fn into_rows(self) -> Vec<RowData> {
        self.rows
    }

    /// Returns true when every row has a non-PLACEHOLDER status.
    ///
    /// After [`finalize_pending`] all rows have non-PLACEHOLDER status,
    /// so this returns `true`.
    pub fn is_complete(&self) -> bool {
        self.rows.iter().all(|r| r.status != PLACEHOLDER)
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
                let status_cell = if r.status == PLACEHOLDER {
                    Cell::from(Span::styled(PLACEHOLDER, style::placeholder_style()))
                } else {
                    Cell::from(Span::styled(
                        r.status.as_str(),
                        style::status_style(&r.status),
                    ))
                };
                Row::new(vec![
                    Cell::from(r.worktree_id.as_str()),
                    Cell::from(r.current_branch.as_str()),
                    status_cell,
                    Cell::from(r.age.as_str()),
                    Cell::from(r.rel_path.as_str()),
                ])
            })
            .collect();

        // #28: fixed proportional widths because the terminal width may change
        // between draws (user resizes). The static layout used for non-TTY and
        // narrow terminals computes column widths from data lengths instead —
        // that's safe because it only runs once, after all statuses are known.
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

/// Drive the Inline Viewport render loop, consuming `(row_index, status)`
/// updates from `rx` until all rows are filled or the sender disconnects.
///
/// The caller is responsible for spawning the producer (typically a
/// `rayon` par_iter inside a `std::thread::scope` that iterates worktrees
/// in parallel and sends results) and for drawing the initial skeleton frame
/// before calling `run` (e.g. `terminal.draw(|f| app.render(f))`). `run`
/// draws the skeleton itself on its first iteration for backward-compat with
/// test callers that do not pre-draw; the duplicate draw is harmless.
///
/// On return, `app.rows` contains final statuses. The viewport exits via
/// `drop(terminal)` which leaves the final frame in the scrollback.
///
/// Returns `Result<(), B::Error>`. For `CrosstermBackend`, `B::Error = io::Error`,
/// and `display.rs` converts it to `crate::error::Result` via the `#[from]` impl on
/// `CwError::Io`. For `TestBackend`, `B::Error = Infallible`, and tests use
/// `.expect(...)` to unwrap.
pub fn run<B: ratatui::backend::Backend>(
    terminal: &mut ratatui::Terminal<B>,
    app: &mut ListApp,
    rx: mpsc::Receiver<(usize, String)>,
) -> Result<(), B::Error> {
    terminal.draw(|f| app.render(f))?;

    // Spec called for `recv_timeout(50ms)` for periodic refresh; we use blocking
    // `recv()` because ratatui handles resize between draws automatically and
    // every status message already triggers a redraw. No tick needed.
    while let Ok((i, status)) = rx.recv() {
        app.set_status(i, status);
        terminal.draw(|f| app.render(f))?;
        if app.is_complete() {
            break;
        }
    }
    // rx.recv() returns Err when the sender drops — all statuses received or
    // producer panicked. Either way the loop exits cleanly.

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
    fn skeleton_frame_shows_placeholder_for_all_rows() {
        let app = ListApp::new(vec![
            sample_row("feat/a", PLACEHOLDER),
            sample_row("feat/b", PLACEHOLDER),
        ]);
        let backend = TestBackend::new(80, 6);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| app.render(f)).unwrap();
        let buf = terminal.backend().buffer().clone();
        let rendered = buffer_to_string(&buf);
        assert!(rendered.contains("feat/a"));
        assert!(rendered.contains("feat/b"));
        assert!(rendered.contains(PLACEHOLDER));
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
            sample_row("feat/a", PLACEHOLDER),
            sample_row("feat/b", PLACEHOLDER),
        ]);
        let backend = TestBackend::new(80, 6);
        let mut terminal = Terminal::new(backend).unwrap();

        let (tx, rx) = std::sync::mpsc::channel();
        // #26: bind the handle and join so the thread is not silently leaked.
        let h = std::thread::spawn(move || {
            tx.send((0, "clean".to_string())).unwrap();
            tx.send((1, "modified".to_string())).unwrap();
        });

        run(&mut terminal, &mut app, rx).expect("list_view::run failed");
        h.join().expect("status producer thread panicked"); // #26: propagate any thread panic to the test runner
        assert_eq!(app.rows()[0].status, "clean");
        assert_eq!(app.rows()[1].status, "modified");
        assert!(app.is_complete());
    }

    #[test]
    fn run_exits_when_sender_drops_with_pending_rows() {
        let mut app = ListApp::new(vec![
            sample_row("feat/a", PLACEHOLDER),
            sample_row("feat/b", PLACEHOLDER),
        ]);
        let backend = TestBackend::new(80, 6);
        let mut terminal = Terminal::new(backend).unwrap();

        let (tx, rx) = std::sync::mpsc::channel();
        // #27: bind the handle and join so the thread is not silently leaked.
        let h = std::thread::spawn(move || {
            tx.send((0, "clean".to_string())).unwrap();
            // Drop tx without sending the second row — simulates panic.
        });

        run(&mut terminal, &mut app, rx).expect("list_view::run failed");
        h.join().expect("status producer thread panicked"); // #27: propagate any thread panic to the test runner
        assert_eq!(app.rows()[0].status, "clean");
        assert_eq!(app.rows()[1].status, PLACEHOLDER); // still pending
        assert!(!app.is_complete());
    }

    /// #27: finalize_pending correctly fills rows that were still PLACEHOLDER
    /// when the producer disconnected (simulates panic or early exit).
    #[test]
    fn run_then_finalize_replaces_pending_after_disconnect() {
        let mut app = ListApp::new(vec![
            sample_row("a", PLACEHOLDER),
            sample_row("b", PLACEHOLDER),
        ]);
        let backend = TestBackend::new(80, 6);
        let mut terminal = Terminal::new(backend).unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let h = std::thread::spawn(move || {
            tx.send((0, "clean".to_string())).unwrap();
            // drop tx without sending row 1 — simulates producer panic
        });
        run(&mut terminal, &mut app, rx).expect("status producer thread panicked");
        h.join().expect("status producer thread panicked");
        let changed = app.finalize_pending("unknown");
        assert!(
            changed,
            "row 1 was still PLACEHOLDER, so changed must be true"
        );
        assert_eq!(app.rows()[0].status, "clean");
        assert_eq!(app.rows()[1].status, "unknown");
    }

    #[test]
    fn finalize_pending_replaces_placeholders() {
        let mut app = ListApp::new(vec![
            sample_row("feat/a", PLACEHOLDER),
            sample_row("feat/b", "clean"),
        ]);
        let changed = app.finalize_pending("unknown");
        assert!(changed, "feat/a was PLACEHOLDER so changed should be true");
        assert_eq!(app.rows()[0].status, "unknown");
        assert_eq!(app.rows()[1].status, "clean"); // unchanged
    }

    // #36: tests for finalize_pending's bool return value (guarded redraw).
    #[test]
    fn finalize_pending_replaces_only_placeholders() {
        let mut app = ListApp::new(vec![
            sample_row("a", "clean"),
            sample_row("b", PLACEHOLDER),
            sample_row("c", "modified"),
        ]);
        let changed = app.finalize_pending("unknown");
        assert!(
            changed,
            "should return true when placeholders were replaced"
        );
        let rows = app.rows();
        assert_eq!(rows[0].status, "clean");
        assert_eq!(rows[1].status, "unknown");
        assert_eq!(rows[2].status, "modified");
    }

    #[test]
    fn finalize_pending_returns_false_when_nothing_pending() {
        let mut app = ListApp::new(vec![sample_row("a", "clean")]);
        assert!(
            !app.finalize_pending("unknown"),
            "should return false when no placeholders present"
        );
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
