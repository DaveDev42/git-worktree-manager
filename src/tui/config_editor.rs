//! `gw config edit` — interactive TUI editor.
//!
//! Lets the user browse every known [`ConfigKey`] alongside the values stored
//! in both scopes (global and repo) and edit either one in place. Saving
//! writes the modified entries back to disk in pretty JSON.
//!
//! Why a TUI and not a "$EDITOR-opens-the-json-file" flow:
//!
//! - We want users to see _both_ scopes at once so they can spot overrides.
//!   A raw-JSON editor only shows one file.
//! - We control the value parser per key, so booleans / numbers / arg arrays
//!   get validated on save instead of producing a JSON file the runtime
//!   silently rejects on next load.
//!
//! Layout (alternate screen, ratatui Crossterm backend):
//!
//!   ┌─ gw config edit ─────────────────────────────────────────┐
//!   │ Scope: ●global  ○repo (.cwconfig.json)         [Tab] swap│
//!   ├──────────────────────────────────────────────────────────┤
//!   │   ai-tool.command       claude                           │
//!   │ ▶ launch.method         tmux                             │
//!   │   …                                                      │
//!   ├──────────────────────────────────────────────────────────┤
//!   │ ↑↓ navigate  Enter edit  Tab swap scope  s save  q quit  │
//!   └──────────────────────────────────────────────────────────┘

use std::io::{stdout, IsTerminal};
use std::path::{Path, PathBuf};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Terminal;
use serde_json::Value;

use crate::error::{CwError, Result};
use crate::git;
use crate::operations::config_ops::{
    global_path, lookup_value, parse_value_for, read_json_or_empty, set_value, unset_value,
    write_json, ConfigKey, Scope,
};

/// Entry point — call from the CLI dispatcher.
pub fn run() -> Result<()> {
    if !stdout().is_terminal() {
        return Err(CwError::Config(
            "gw config edit needs a TTY (run it in an interactive terminal)".to_string(),
        ));
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let global = global_path();
    let repo = git::get_repo_root(Some(&cwd))
        .ok()
        .map(|r| r.join(".cwconfig.json"));

    let app_state = AppState::load(&global, repo.as_deref())?;
    let outcome = run_loop(app_state)?;
    if let Some(saved) = outcome {
        for (label, count) in saved.summary() {
            if count > 0 {
                println!(" {}: {} key(s) updated", label, count);
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Application state
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct AppState {
    /// Working copy of the global file (edits accumulate here until save).
    global_value: Value,
    /// Working copy of the repo file (None when not in a repo).
    repo_value: Option<Value>,

    global_path: PathBuf,
    repo_path: Option<PathBuf>,

    cursor: usize,
    active: Scope,
    dirty_global: bool,
    dirty_repo: bool,
    status: String,
    /// Modal input prompt when editing a value. `None` when navigating.
    editing: Option<EditState>,
    quit: bool,
}

#[derive(Debug)]
struct EditState {
    key: ConfigKey,
    scope: Scope,
    buffer: String,
    error: Option<String>,
}

impl AppState {
    fn load(global: &Path, repo: Option<&Path>) -> Result<Self> {
        let global_value = read_json_or_empty(global)?;
        let repo_value = match repo {
            Some(p) => Some(read_json_or_empty(p)?),
            None => None,
        };
        // Always start in global scope; users hit Tab to switch when a
        // repo is loaded (and we no-op the toggle when it isn't).
        Ok(Self {
            global_value,
            repo_value,
            global_path: global.to_path_buf(),
            repo_path: repo.map(Path::to_path_buf),
            cursor: 0,
            active: Scope::Global,
            dirty_global: false,
            dirty_repo: false,
            status: String::from("Tab: swap scope · Enter: edit · s: save · q: quit"),
            editing: None,
            quit: false,
        })
    }

    fn keys() -> &'static [ConfigKey] {
        ConfigKey::ALL
    }

    fn active_value(&self) -> Option<&Value> {
        match self.active {
            Scope::Global => Some(&self.global_value),
            Scope::Repo => self.repo_value.as_ref(),
        }
    }

    fn current_key(&self) -> ConfigKey {
        Self::keys()[self.cursor]
    }

    fn move_cursor(&mut self, delta: i32) {
        let len = Self::keys().len() as i32;
        if len == 0 {
            return;
        }
        let next = (self.cursor as i32 + delta).rem_euclid(len);
        self.cursor = next as usize;
    }

    fn toggle_scope(&mut self) {
        if self.repo_value.is_none() {
            self.status = "Not inside a git repo — repo scope unavailable".to_string();
            return;
        }
        self.active = self.active.other();
    }

    /// Start editing the value under cursor in the active scope.
    fn begin_edit(&mut self) {
        if matches!(self.active, Scope::Repo) && self.repo_value.is_none() {
            self.status = "Not inside a git repo — cannot edit repo scope".to_string();
            return;
        }
        let key = self.current_key();
        let buffer = match self.active {
            Scope::Global => lookup_value(&self.global_value, key)
                .map(value_to_input_string)
                .unwrap_or_default(),
            Scope::Repo => self
                .repo_value
                .as_ref()
                .and_then(|v| lookup_value(v, key))
                .map(value_to_input_string)
                .unwrap_or_default(),
        };
        self.editing = Some(EditState {
            key,
            scope: self.active,
            buffer,
            error: None,
        });
    }

    /// Apply the value currently in the edit buffer to the working tree.
    fn commit_edit(&mut self) {
        // Take editing out so we can borrow self mutably for the apply step
        // without holding a borrow on `editing` at the same time.
        let edit = match self.editing.take() {
            Some(e) => e,
            None => return,
        };
        let trimmed = edit.buffer.trim();
        let result = if trimmed.is_empty() {
            // Empty input = unset this key in the active scope.
            self.apply_unset(edit.key, edit.scope);
            Ok(())
        } else {
            parse_value_for(edit.key, &edit.buffer)
                .and_then(|v| self.apply_set(edit.key, edit.scope, v))
        };
        match result {
            Ok(()) => {
                self.status = format!(
                    "Updated {} in [{}] (unsaved)",
                    edit.key.name(),
                    edit.scope.label()
                );
            }
            Err(e) => {
                // Put the edit back so the user can fix it.
                self.editing = Some(EditState {
                    error: Some(format!("{e}")),
                    ..edit
                });
            }
        }
    }

    fn cancel_edit(&mut self) {
        self.editing = None;
    }

    fn apply_set(&mut self, key: ConfigKey, scope: Scope, value: Value) -> Result<()> {
        match scope {
            Scope::Global => {
                set_value(&mut self.global_value, key, value)?;
                self.dirty_global = true;
            }
            Scope::Repo => {
                let v = self.repo_value.as_mut().ok_or_else(|| {
                    CwError::Config("repo scope unavailable (not in a git repo)".to_string())
                })?;
                set_value(v, key, value)?;
                self.dirty_repo = true;
            }
        }
        Ok(())
    }

    fn apply_unset(&mut self, key: ConfigKey, scope: Scope) {
        match scope {
            Scope::Global => {
                unset_value(&mut self.global_value, key);
                self.dirty_global = true;
            }
            Scope::Repo => {
                if let Some(v) = self.repo_value.as_mut() {
                    unset_value(v, key);
                    self.dirty_repo = true;
                }
            }
        }
    }

    fn save(&mut self) -> Result<SaveSummary> {
        let mut summary = SaveSummary::default();
        if self.dirty_global {
            write_json(&self.global_path, &self.global_value)?;
            summary.global = count_set_keys(&self.global_value);
            self.dirty_global = false;
        }
        if self.dirty_repo {
            if let (Some(p), Some(v)) = (self.repo_path.as_ref(), self.repo_value.as_ref()) {
                write_json(p, v)?;
                summary.repo = count_set_keys(v);
                self.dirty_repo = false;
            }
        }
        self.status = "Saved.".to_string();
        Ok(summary)
    }
}

/// Count keys that are currently stored in `root` (across all known
/// [`ConfigKey`]s). Used by `save`'s summary line.
fn count_set_keys(root: &Value) -> usize {
    ConfigKey::ALL
        .iter()
        .filter(|k| lookup_value(root, **k).is_some())
        .count()
}

/// Format a JSON value back into something the user can re-edit. Strings come
/// out unquoted; arrays render as space-joined tokens when every element is a
/// plain string (the common case for `ai-tool.args`).
fn value_to_input_string(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::Array(a) => {
            if a.iter().all(|e| e.is_string()) {
                a.iter()
                    .map(|e| e.as_str().unwrap_or("").to_string())
                    .collect::<Vec<_>>()
                    .join(" ")
            } else {
                v.to_string()
            }
        }
        Value::Object(_) => v.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Save summary returned to the dispatcher
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct SaveSummary {
    pub global: usize,
    pub repo: usize,
}

impl SaveSummary {
    fn summary(&self) -> [(&'static str, usize); 2] {
        [("global", self.global), ("repo", self.repo)]
    }
}

// ---------------------------------------------------------------------------
// Event loop
// ---------------------------------------------------------------------------

fn run_loop(mut state: AppState) -> Result<Option<SaveSummary>> {
    enable_raw_mode().map_err(io_err)?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen).map_err(io_err)?;
    crate::tui::mark_ratatui_active();

    let backend = CrosstermBackend::new(out);
    let mut terminal = Terminal::new(backend).map_err(io_err)?;
    let result = (|| -> Result<Option<SaveSummary>> {
        let mut last_save: Option<SaveSummary> = None;
        loop {
            terminal.draw(|f| render(f, &state)).map_err(io_err)?;
            if state.quit {
                break;
            }
            let evt = event::read().map_err(io_err)?;
            if let Event::Key(k) = evt {
                if let Some(s) = handle_key(&mut state, k)? {
                    last_save = Some(s);
                }
            }
        }
        Ok(last_save)
    })();

    // Always restore terminal state, even on error.
    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    drop(terminal);
    crate::tui::mark_ratatui_inactive();

    result
}

fn io_err(e: std::io::Error) -> CwError {
    CwError::Other(format!("terminal io: {}", e))
}

fn handle_key(state: &mut AppState, key: KeyEvent) -> Result<Option<SaveSummary>> {
    if state.editing.is_some() {
        handle_edit_key(state, key);
        return Ok(None);
    }
    // Ctrl-C always quits without saving, matching every other gw TUI.
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        state.quit = true;
        return Ok(None);
    }
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => state.quit = true,
        KeyCode::Up | KeyCode::Char('k') => state.move_cursor(-1),
        KeyCode::Down | KeyCode::Char('j') => state.move_cursor(1),
        KeyCode::Tab => state.toggle_scope(),
        KeyCode::Enter => state.begin_edit(),
        KeyCode::Char('s') => {
            let s = state.save()?;
            return Ok(Some(s));
        }
        _ => {}
    }
    Ok(None)
}

fn handle_edit_key(state: &mut AppState, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => state.cancel_edit(),
        KeyCode::Enter => state.commit_edit(),
        KeyCode::Backspace => {
            if let Some(e) = state.editing.as_mut() {
                e.buffer.pop();
                e.error = None;
            }
        }
        KeyCode::Char(c) => {
            // Ctrl-C inside the edit prompt cancels the edit; matches dialoguer
            // and "any reasonable text input" expectations.
            if key.modifiers.contains(KeyModifiers::CONTROL) && (c == 'c' || c == 'C') {
                state.cancel_edit();
                return;
            }
            if let Some(e) = state.editing.as_mut() {
                e.buffer.push(c);
                e.error = None;
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn render(f: &mut ratatui::Frame, state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(2),
        ])
        .split(f.area());

    render_header(f, chunks[0], state);
    render_body(f, chunks[1], state);
    render_footer(f, chunks[2], state);

    if let Some(edit) = &state.editing {
        render_edit_modal(f, edit);
    }
}

fn render_header(f: &mut ratatui::Frame, area: Rect, state: &AppState) {
    let dim_when_unavailable = state.repo_value.is_none();
    let active_global = matches!(state.active, Scope::Global);
    let active_repo = matches!(state.active, Scope::Repo);

    let global_label = if active_global {
        Span::styled("● global", Style::new().add_modifier(Modifier::BOLD))
    } else {
        Span::raw("○ global")
    };
    let repo_label = if dim_when_unavailable {
        Span::styled("○ repo (n/a)", Style::new().add_modifier(Modifier::DIM))
    } else if active_repo {
        Span::styled("● repo", Style::new().add_modifier(Modifier::BOLD))
    } else {
        Span::raw("○ repo")
    };

    let line = Line::from(vec![
        Span::raw("Scope: "),
        global_label,
        Span::raw("   "),
        repo_label,
        Span::raw("    "),
        Span::styled(
            current_path_hint(state),
            Style::new().add_modifier(Modifier::DIM),
        ),
    ]);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" gw config edit ");
    let para = Paragraph::new(line).block(block);
    f.render_widget(para, area);
}

fn current_path_hint(state: &AppState) -> String {
    let p = match state.active {
        Scope::Global => Some(state.global_path.as_path()),
        Scope::Repo => state.repo_path.as_deref(),
    };
    p.map(|p| p.display().to_string()).unwrap_or_default()
}

fn render_body(f: &mut ratatui::Frame, area: Rect, state: &AppState) {
    let keys = AppState::keys();
    let key_col = keys.iter().map(|k| k.name().len()).max().unwrap_or(0);

    let mut lines: Vec<Line> = Vec::with_capacity(keys.len());
    for (i, key) in keys.iter().enumerate() {
        let v = match state.active_value() {
            Some(root) => lookup_value(root, *key),
            None => None,
        };
        let value_str = match v {
            Some(v) => value_to_input_string(v),
            None => "(unset)".to_string(),
        };
        let key_text = format!("{key:<key_col$}", key = key.name());
        let marker = if i == state.cursor { "▶ " } else { "  " };
        let style = if i == state.cursor {
            Style::new().add_modifier(Modifier::REVERSED)
        } else {
            Style::new()
        };
        lines.push(Line::from(vec![
            Span::raw(marker),
            Span::styled(format!("{key_text}  {value_str}"), style),
        ]));
    }

    let title = format!(" {} ", state.active.label());
    let block = Block::default().borders(Borders::ALL).title(title);
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_footer(f: &mut ratatui::Frame, area: Rect, state: &AppState) {
    let dirty = state.dirty_global || state.dirty_repo;
    let mut spans = vec![Span::raw(state.status.clone())];
    if dirty {
        spans.push(Span::raw("   "));
        spans.push(Span::styled(
            "[unsaved]",
            Style::new().add_modifier(Modifier::BOLD),
        ));
    }
    let line = Line::from(spans);
    f.render_widget(Paragraph::new(line), area);
}

fn render_edit_modal(f: &mut ratatui::Frame, edit: &EditState) {
    let area = centered_rect(60, 7, f.area());
    f.render_widget(Clear, area);

    let title = format!(" edit {} [{}] ", edit.key.name(), edit.scope.label());
    let mut lines = vec![
        Line::from(Span::raw(format!("{}_", edit.buffer))),
        Line::from(Span::styled(
            "Enter: commit · Esc: cancel · empty = unset",
            Style::new().add_modifier(Modifier::DIM),
        )),
    ];
    if let Some(err) = &edit.error {
        lines.push(Line::from(Span::styled(
            err.clone(),
            Style::new().add_modifier(Modifier::BOLD),
        )));
    }
    let block = Block::default().borders(Borders::ALL).title(title);
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let w = width.min(area.width.saturating_sub(2));
    let h = height.min(area.height.saturating_sub(2));
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    Rect {
        x,
        y,
        width: w,
        height: h,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    fn fresh_state() -> AppState {
        // Synthetic state: empty global, no repo. We bypass `load` since the
        // tests want to drive the state machine directly.
        AppState {
            global_value: Value::Object(serde_json::Map::new()),
            repo_value: None,
            global_path: PathBuf::from("/tmp/none-global.json"),
            repo_path: None,
            cursor: 0,
            active: Scope::Global,
            dirty_global: false,
            dirty_repo: false,
            status: String::new(),
            editing: None,
            quit: false,
        }
    }

    #[test]
    fn move_cursor_wraps_in_both_directions() {
        let mut s = fresh_state();
        s.move_cursor(-1);
        assert_eq!(s.cursor, ConfigKey::ALL.len() - 1);
        s.move_cursor(1);
        assert_eq!(s.cursor, 0);
    }

    #[test]
    fn toggle_scope_blocked_outside_repo() {
        let mut s = fresh_state();
        s.toggle_scope();
        assert_eq!(s.active, Scope::Global);
        assert!(s.status.contains("not inside") || s.status.to_lowercase().contains("repo"));
    }

    #[test]
    fn begin_edit_seeds_buffer_with_current_value() {
        let mut s = fresh_state();
        s.global_value = json!({"ai_tool": {"command": "codex"}});
        // cursor=0 → ai-tool.command
        s.begin_edit();
        let edit = s.editing.as_ref().unwrap();
        assert_eq!(edit.buffer, "codex");
        assert_eq!(edit.scope, Scope::Global);
    }

    #[test]
    fn commit_edit_updates_value_and_marks_dirty() {
        let mut s = fresh_state();
        s.begin_edit();
        s.editing.as_mut().unwrap().buffer = "codex".to_string();
        s.commit_edit();
        assert!(s.dirty_global);
        assert_eq!(
            lookup_value(&s.global_value, ConfigKey::AiToolCommand).unwrap(),
            &Value::String("codex".into())
        );
        assert!(s.editing.is_none());
    }

    #[test]
    fn commit_edit_with_empty_buffer_unsets() {
        let mut s = fresh_state();
        s.global_value = json!({"ai_tool": {"command": "codex"}});
        s.begin_edit();
        s.editing.as_mut().unwrap().buffer = "  ".to_string();
        s.commit_edit();
        assert!(s.dirty_global);
        assert!(lookup_value(&s.global_value, ConfigKey::AiToolCommand).is_none());
    }

    #[test]
    fn commit_edit_invalid_value_keeps_editing_open() {
        let mut s = fresh_state();
        // cursor=2 → ai-tool.guard (boolean)
        s.cursor = 2;
        s.begin_edit();
        s.editing.as_mut().unwrap().buffer = "definitely-not-bool".to_string();
        s.commit_edit();
        assert!(s.editing.is_some(), "should remain in edit mode on error");
        assert!(s.editing.as_ref().unwrap().error.is_some());
        assert!(!s.dirty_global);
    }

    #[test]
    fn save_persists_global_changes() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("config.json");
        let mut s = fresh_state();
        s.global_path = p.clone();
        s.global_value = json!({"ai_tool": {"command": "codex"}});
        s.dirty_global = true;

        let summary = s.save().unwrap();
        assert_eq!(summary.global, 1);
        assert!(!s.dirty_global);

        let read = std::fs::read_to_string(&p).unwrap();
        assert!(read.contains("\"command\""));
        assert!(read.contains("\"codex\""));
    }

    #[test]
    fn save_skips_repo_when_no_repo_loaded() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("config.json");
        let mut s = fresh_state();
        s.global_path = p;
        s.dirty_repo = true;

        // No panic, no write attempted; save still succeeds.
        let summary = s.save().unwrap();
        assert_eq!(summary.repo, 0);
    }
}
