/// Display and information operations for git-worktree-manager.
///
use std::path::Path;
use std::sync::mpsc;

// #35: two `console`-related imports are intentional:
// - `console::style` is from the external `console` crate (ANSI styling)
// - `crate::console as cwconsole` is this crate's own console helpers (terminal_width, etc.)
// Aliasing avoids a name collision that would shadow the crate's `style` function.
use console::style;
use ratatui::{backend::CrosstermBackend, Terminal, TerminalOptions, Viewport};

use crate::console as cwconsole;
use crate::constants::{
    format_config_key, path_age_days, sanitize_branch_name, CONFIG_KEY_BASE_BRANCH,
    CONFIG_KEY_INTENDED_BRANCH,
};
use crate::error::Result;
use crate::git;

use rayon::prelude::*;

use super::pr_cache::PrCache;

/// Minimum terminal width for table layout; below this, use compact layout.
const MIN_TABLE_WIDTH: usize = 100;

// TODO(perf): hoist `base_branch` and `cwd_canon` lookups out of `get_worktree_status`
// to avoid N×git-config calls. ~6 call sites; consider a `WorktreeContext` struct.
// (Deferred in this PR to keep diff scope manageable.)

/// Determine the status of a worktree.
///
/// Status priority: stale > busy > active > merged > pr-open > modified > clean
///
/// Merge detection strategy:
/// 1. Cached `gh pr list` (primary) — works with all merge strategies (merge
///    commit, squash merge, rebase merge) because GitHub tracks PR state
///    independently of commit SHAs. One `gh` call per repo, cached 60 s.
/// 2. `git branch --merged` (fallback) — only works when commit SHAs are
///    preserved (merge commit strategy). Used when `gh` is not available.
///
/// See `pr_cache::PrCache` for the batched fetch and TTL details.
pub fn get_worktree_status(
    path: &Path,
    repo: &Path,
    branch: Option<&str>,
    pr_cache: &PrCache,
) -> String {
    if !path.exists() {
        return "stale".to_string();
    }

    // Busy beats "active": another session (claude, shell, editor) holds this
    // worktree. The current process and its ancestors are excluded inside
    // detect_busy_lockfile_only so the caller's own shell does not self-report.
    //
    // Uses the lockfile-only fast path: the full cwd scan (lsof / /proc walk)
    // takes ~1.5s on macOS and dominates `gw list` latency. This narrows
    // exclusion to ancestors only (no siblings) since the fast path must
    // avoid `self_siblings`, which internally triggers the cwd scan.
    // Destructive commands (`gw rm`) still use the full `detect_busy`.
    if !crate::operations::busy::detect_busy_lockfile_only(path).is_empty() {
        return "busy".to_string();
    }

    // Also flag worktrees occupied by an active Claude Code session.
    // Shares the two-stage gate (jsonl event + live `claude` process) with
    // `detect_busy_tiered` via `busy::active_claude_sessions` so the two
    // surfaces cannot drift. The process scan is OnceLock-cached.
    if crate::operations::busy::active_claude_sessions(path).is_some() {
        return "busy".to_string();
    }

    // Check if cwd is inside this worktree. Canonicalize both sides so that
    // symlink skew (e.g. macOS /var vs /private/var) does not miss a match.
    if let Ok(cwd) = std::env::current_dir() {
        let cwd_canon = cwd.canonicalize().unwrap_or(cwd);
        let path_canon = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        if cwd_canon.starts_with(&path_canon) {
            return "active".to_string();
        }
    }

    // Check merge/PR status if branch name is available
    if let Some(branch_name) = branch {
        let base_branch = {
            let key = format_config_key(CONFIG_KEY_BASE_BRANCH, branch_name);
            git::get_config(&key, Some(repo))
                .unwrap_or_else(|| git::detect_default_branch(Some(repo)))
        };

        // Primary: cached PR state from a single `gh pr list` call.
        if let Some(state) = pr_cache.state(branch_name) {
            match state {
                super::pr_cache::PrState::Merged => return "merged".to_string(),
                super::pr_cache::PrState::Open => return "pr-open".to_string(),
                // Closed/Other: fall through to git-based merge detection
                _ => {}
            }
        }

        // Fallback: git branch --merged (only works for merge-commit strategy)
        // Used when `gh` is not installed or no PR was created
        if git::is_branch_merged(branch_name, &base_branch, Some(repo)) {
            return "merged".to_string();
        }
    }

    // Check for uncommitted changes
    if let Ok(result) = git::git_command(&["status", "--porcelain"], Some(path), false, true) {
        if result.returncode == 0 && !result.stdout.trim().is_empty() {
            return "modified".to_string();
        }
    }

    "clean".to_string()
}

/// Format age in days to human-readable string.
pub fn format_age(age_days: f64) -> String {
    if age_days < 1.0 {
        let hours = (age_days * 24.0) as i64;
        if hours > 0 {
            format!("{}h ago", hours)
        } else {
            "just now".to_string()
        }
    } else if age_days < 7.0 {
        format!("{}d ago", age_days as i64)
    } else if age_days < 30.0 {
        format!("{}w ago", (age_days / 7.0) as i64)
    } else if age_days < 365.0 {
        format!("{}mo ago", (age_days / 30.0) as i64)
    } else {
        format!("{}y ago", (age_days / 365.0) as i64)
    }
}

/// Compose a single row for the `gw rm -i` multi-select TUI.
///
/// Columns, left to right, separated by one space:
///   branch (padded to `branch_col`) | age (padded to 9) | busy (7, colored) | path
///
/// The busy column carries an ANSI-colored `[busy]` token when `busy` is true,
/// or 7 spaces when false. `arrow_select::visible_len` is ANSI-aware, so the
/// colored and plain variants have identical visible width.
///
/// The path column is appended verbatim. The caller is expected to run the
/// returned string through `arrow_select::truncate` to cap line width; that
/// truncation clips the trailing path column, which is the correct behavior
/// per the row-decorations spec (badges and age must survive, path may shrink).
pub fn format_selector_row(
    branch: &str,
    age: &str,
    busy: bool,
    path: &str,
    branch_col: usize,
) -> String {
    const AGE_COL: usize = 9;
    const BUSY_COL: usize = 7;
    let busy_cell: String = if busy {
        // "[busy]" is 6 visible chars; pad to BUSY_COL with one trailing space.
        format!("{} ", style("[busy]").yellow())
    } else {
        " ".repeat(BUSY_COL)
    };
    format!(
        "{branch:<branch_col$} {age:<AGE_COL$} {busy_cell}{path}",
        branch = branch,
        age = age,
        busy_cell = busy_cell,
        path = path,
        branch_col = branch_col,
        AGE_COL = AGE_COL,
    )
}

/// Compute age string for a path.
fn path_age_str(path: &Path) -> String {
    if !path.exists() {
        return String::new();
    }
    path_age_days(path).map(format_age).unwrap_or_default()
}

/// Collected worktree data row for display.
struct WorktreeRow {
    worktree_id: String,
    current_branch: String,
    status: String,
    age: String,
    rel_path: String,
    /// Worktree path; retained so post-render `print_busy_details` can
    /// scan busy rows without re-parsing `git worktree list`.
    path: std::path::PathBuf,
}

/// Serial-prep input passed to status computation.
/// Shares all fields with `WorktreeRow` except `status` (filled in by the
/// parallel worker).
#[derive(Clone)]
struct RowInput {
    path: std::path::PathBuf,
    current_branch: String,
    worktree_id: String,
    age: String,
    rel_path: String,
}

impl RowInput {
    fn into_row(self, status: String) -> WorktreeRow {
        WorktreeRow {
            worktree_id: self.worktree_id,
            current_branch: self.current_branch,
            status,
            age: self.age,
            rel_path: self.rel_path,
            path: self.path,
        }
    }
}

/// Prewarm the two `lsof`-backed caches (`busy::cwd_scan`,
/// `claude_process::snapshot`) on detached background threads so they run
/// concurrently with each other and with the foreground status loop.
/// Later callers (`get_worktree_status`, `print_busy_details`) hit the
/// cache instead of paying the lsof round-trip serially.
///
/// Detached threads: the join handles are dropped immediately. Both
/// workers only mutate process-static `OnceLock`s, so a slow/stuck
/// thread does not block process exit any more than a slow lsof already
/// would; the foreground caller will race against them via
/// `OnceLock::get_or_init`. We don't `join` here because the prewarm is
/// best-effort — if it's still running when a caller hits the cache,
/// `get_or_init` blocks once and continues. If the thread completes
/// first, callers find the cache already populated.
fn prewarm_busy_caches() {
    std::thread::spawn(crate::operations::busy::prewarm_cwd_scan);
    std::thread::spawn(crate::operations::claude_process::prewarm);
}

/// List all worktrees, grouped by repository, using cwd-based scope discovery.
pub fn list_worktrees(no_cache: bool) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let scope = crate::scope::discover_scope(&cwd)?;

    if scope.is_empty() {
        println!("  {}\n", style("No worktrees found.").dim());
        return Ok(());
    }

    // Group by repo_root, preserving first-seen order.
    // Using Vec<(PathBuf, Vec<(String, PathBuf)>)> instead of HashMap to keep
    // a stable, deterministic order for multi-repo output.
    let mut groups: Vec<(std::path::PathBuf, Vec<(String, std::path::PathBuf)>)> = Vec::new();
    for w in scope.worktrees() {
        let key = &w.repo_root;
        let entry = match groups.iter_mut().find(|(k, _)| k == key) {
            Some(e) => e,
            None => {
                groups.push((key.clone(), Vec::new()));
                groups.last_mut().unwrap()
            }
        };
        // Re-derive (branch_raw, path) tuple shape that the existing rendering
        // pipeline expects. For detached HEAD, use "(detached)" so downstream
        // normalize_branch_name keeps working.
        let branch_raw = w.branch.clone().unwrap_or_else(|| "(detached)".to_string());
        entry.1.push((branch_raw, w.path.clone()));
    }

    for (i, (repo, worktrees)) in groups.iter().enumerate() {
        if i > 0 {
            println!(); // separator between sections
        }
        render_repo_section(repo, worktrees, no_cache)?;
    }
    Ok(())
}

/// Render a single repository's worktree list section.
///
/// This is the extracted body of the original `list_worktrees`, parameterised
/// over `(repo, worktrees)` so the outer function can call it once per repo
/// family found by `scope::discover_scope`.
fn render_repo_section(
    repo: &std::path::Path,
    worktrees: &[(String, std::path::PathBuf)],
    no_cache: bool,
) -> Result<()> {
    println!(
        "\n{}  {}\n",
        style("Worktrees for repository:").cyan().bold(),
        repo.display()
    );

    let pr_cache = PrCache::load_or_fetch(repo, no_cache);

    // Serial prep: cheap local work. Keep single-threaded for clarity.
    let inputs: Vec<RowInput> = worktrees
        .iter()
        .map(|(branch, path)| {
            let current_branch = git::normalize_branch_name(branch).to_string();
            let rel_path = pathdiff::diff_paths(path, repo)
                .map(|p: std::path::PathBuf| p.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string_lossy().to_string());
            let age = path_age_str(path);
            let intended_branch = lookup_intended_branch(repo, &current_branch, path);
            let worktree_id = intended_branch.unwrap_or_else(|| current_branch.clone());
            RowInput {
                path: path.clone(),
                current_branch,
                worktree_id,
                age,
                rel_path,
            }
        })
        .collect();

    // A repo section with zero worktrees produces an empty table — acceptable
    // in practice since a repo always has at least the main worktree.
    // (The outer scope-level empty check handles the truly empty case.)

    prewarm_busy_caches();

    let is_tty = crate::tui::stdout_is_tty();
    // #18/#33/#35: cache terminal_width() once — used in both the progressive/static
    // branch decision and the post-render print guard.
    let term_width = cwconsole::terminal_width();
    // #35: extract narrow so the two places that check MIN_TABLE_WIDTH share
    // a single bool and cannot drift out of sync.
    let narrow = term_width < MIN_TABLE_WIDTH;
    let use_progressive = is_tty && !narrow;

    let rows: Vec<WorktreeRow> = if use_progressive {
        render_rows_progressive(repo, &pr_cache, inputs)?
    } else {
        // rayon borrows &pr_cache across workers via the type system.
        inputs
            .into_par_iter()
            .map(|i| {
                let status = get_worktree_status(&i.path, repo, Some(&i.current_branch), &pr_cache);
                i.into_row(status)
            })
            .collect()
    };

    // In the TTY+wide path the Inline Viewport has already drawn the table.
    // In the static path (narrow terminal or non-TTY) we still need to print.
    if !use_progressive {
        if narrow {
            print_worktree_compact(&rows);
        } else {
            print_worktree_table(&rows);
        }
    }

    // Footer is printed via println! after the Inline Viewport drops, so it
    // appears below the table in scrollback. Alignment is correct for the
    // static path; in the TTY path the viewport already committed the table
    // rows and the footer follows naturally. Using terminal.insert_before()
    // could align it inside the viewport, but the current behaviour is
    // acceptable and avoids extra ratatui complexity.
    print_busy_details(&rows);
    print_summary_footer(&rows);

    println!();
    Ok(())
}

/// RAII guard: drops the terminal before calling `ratatui::restore()`.
/// Ensures terminal modes are restored deterministically even on early return
/// or panic, without relying on a closure-then-restore pattern.
///
/// #19: concrete backend type avoids unnecessary generics — this guard is only
/// ever created for the crossterm+stdout path used in `render_rows_progressive`.
type CrosstermTerminal = ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>;

/// Wraps a ratatui Terminal with deterministic cleanup.
///
/// # Contract
/// - The caller must call `mark_ratatui_active()` before constructing the terminal.
/// - On Drop, this guard drops the terminal first, then calls `mark_ratatui_inactive()`
///   followed by `ratatui::restore()`.
struct TerminalGuard(Option<CrosstermTerminal>);

impl TerminalGuard {
    fn new(terminal: CrosstermTerminal) -> Self {
        // #1/#20: flag is already set by the caller before Terminal::with_options;
        // the caller's error path calls mark_ratatui_inactive if construction fails.
        // Here we just store the terminal — the flag is already live.
        Self(Some(terminal))
    }

    fn as_mut(&mut self) -> &mut CrosstermTerminal {
        self.0.as_mut().expect("terminal already taken")
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = self.0.take(); // drop terminal first, releasing raw mode if any
        ratatui::restore();
        // #20: clear the panic-hook flag after restore — a subsequent panic
        // (unlikely but possible) must not try to restore a non-existent terminal.
        crate::tui::mark_ratatui_inactive();
    }
}

fn render_rows_progressive(
    repo: &std::path::Path,
    pr_cache: &PrCache,
    inputs: Vec<RowInput>,
) -> Result<Vec<WorktreeRow>> {
    // Build skeleton app.
    let row_data: Vec<crate::tui::list_view::RowData> = inputs
        .iter()
        .map(|i| crate::tui::list_view::RowData {
            worktree_id: i.worktree_id.clone(),
            current_branch: i.current_branch.clone(),
            status: crate::tui::list_view::PLACEHOLDER.to_string(),
            age: i.age.clone(),
            rel_path: i.rel_path.clone(),
        })
        .collect();
    let mut app = crate::tui::list_view::ListApp::new(row_data);

    // `+2` accounts for the header row plus a trailing blank line. Borders are
    // disabled (`Borders::NONE`); the spec's `+4` figure assumed bordered layout.
    let viewport_height = u16::try_from(inputs.len())
        .unwrap_or(u16::MAX)
        .saturating_add(2)
        .max(3);

    let stdout = std::io::stdout();
    let backend = CrosstermBackend::new(stdout);
    // #1/#5: mark active BEFORE construction so the panic hook fires correctly
    // if Terminal::with_options itself panics. If it returns Err or panics, we
    // clear the flag before propagating so a non-ratatui panic later is not
    // mishandled.
    // Restore is idempotent — if construction fails or panics, the panic hook
    // may still call `ratatui::restore()`, which is documented safe.
    crate::tui::mark_ratatui_active();
    let terminal = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        Terminal::with_options(
            backend,
            TerminalOptions {
                viewport: Viewport::Inline(viewport_height),
            },
        )
    })) {
        Ok(Ok(t)) => t,
        Ok(Err(e)) => {
            crate::tui::mark_ratatui_inactive();
            return Err(e.into());
        }
        Err(panic) => {
            crate::tui::mark_ratatui_inactive();
            std::panic::resume_unwind(panic);
        }
    };
    let mut guard = TerminalGuard::new(terminal);
    // Note: no test exercises a panicking Terminal::with_options. The double-restore
    // path (panic hook + TerminalGuard::Drop) is documented safe in ratatui.

    // Producer: parallel per-worktree status computation on a dedicated OS
    // thread; rayon parallelism is used within that thread.
    //
    // Uses std::thread::scope so the consumer (list_view::run) on the main
    // thread can interleave with the producer thread, giving true progressive
    // rendering. Producer panics are caught by the scope's join-on-exit and
    // the sweep below promotes remaining "..." placeholders to "unknown".
    //
    // Uses rayon's default pool (CPU cores). Each worker spawns `git`
    // subprocesses, which is I/O-bound but small enough that oversubscription
    // doesn't help.
    let (tx, rx) = mpsc::channel();

    // Draw skeleton immediately so the user sees the table even before any
    // status computations finish. `list_view::run` would draw this on its
    // first iteration, but for very fast producers (small repos) the rows
    // can fill before that initial draw.
    guard.as_mut().draw(|f| app.render(f))?;

    // Retain paths in row order so the post-render `print_busy_details` block
    // can scan busy rows. The producer takes `inputs` by move into the worker
    // thread, so we capture the paths up-front.
    let paths: Vec<std::path::PathBuf> = inputs.iter().map(|i| i.path.clone()).collect();

    // `thread::scope` blocks until all spawned threads finish (when the closure
    // returns). The explicit `producer.join()` here is solely to extract the
    // panic payload for diagnostics; the actual join would happen automatically
    // at scope exit.
    std::thread::scope(|s| -> Result<()> {
        let producer = s.spawn(move || {
            inputs
                .par_iter()
                .enumerate()
                .for_each_with(tx, |tx, (i, input)| {
                    let status = get_worktree_status(
                        &input.path,
                        repo,
                        Some(&input.current_branch),
                        pr_cache,
                    );
                    let _ = tx.send((i, status));
                });
        });

        let run_result = crate::tui::list_view::run(guard.as_mut(), &mut app, rx);
        let producer_result = producer.join();
        if let Err(panic) = producer_result {
            // #3: extract a readable message from the panic payload.
            let msg = panic
                .downcast_ref::<&str>()
                .map(|s| (*s).to_string())
                .or_else(|| panic.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "non-string panic payload".to_string());
            eprintln!(
                "warning: status producer thread panicked, some rows may show \"unknown\": {}",
                msg
            );
        }
        run_result.map_err(crate::error::CwError::from)
    })?;

    // Defensive sweep: if the producer panicked, some rows may still carry
    // the skeleton placeholder. Promote those to a visible "unknown" status
    // so the footer summary doesn't count the placeholder literal.
    // #5/#39: only redraw when something actually changed to avoid adding a
    // duplicate table frame to scrollback. finalize_pending returns true iff
    // at least one placeholder was replaced.
    if app.finalize_pending("unknown") {
        guard.as_mut().draw(|f| app.render(f))?;
    }

    Ok(app
        .into_rows()
        .into_iter()
        .zip(paths)
        .map(|(r, path)| WorktreeRow {
            worktree_id: r.worktree_id,
            current_branch: r.current_branch,
            status: r.status,
            age: r.age,
            rel_path: r.rel_path,
            path,
        })
        .collect())
}

// Note: `WorktreeRow` is no longer derived `From<RowData>`. The path field
// is plumbed through the zip in `render_rows_progressive` so the busy-details
// printer can recover the worktree path post-render.

/// Look up the intended branch for a worktree via git config metadata.
fn lookup_intended_branch(repo: &Path, current_branch: &str, path: &Path) -> Option<String> {
    // Try direct lookup
    let key = format_config_key(CONFIG_KEY_INTENDED_BRANCH, current_branch);
    if let Some(intended) = git::get_config(&key, Some(repo)) {
        return Some(intended);
    }

    // Search all intended branch metadata
    let result = git::git_command(
        &[
            "config",
            "--local",
            "--get-regexp",
            r"^worktree\..*\.intendedBranch",
        ],
        Some(repo),
        false,
        true,
    )
    .ok()?;

    if result.returncode != 0 {
        return None;
    }

    let repo_name = repo.file_name()?.to_string_lossy().to_string();

    for line in result.stdout.trim().lines() {
        let parts: Vec<&str> = line.splitn(2, char::is_whitespace).collect();
        if parts.len() == 2 {
            let key_parts: Vec<&str> = parts[0].split('.').collect();
            if key_parts.len() >= 2 {
                let branch_from_key = key_parts[1];
                let expected_path_name =
                    format!("{}-{}", repo_name, sanitize_branch_name(branch_from_key));
                if let Some(name) = path.file_name() {
                    if name.to_string_lossy() == expected_path_name {
                        return Some(parts[1].to_string());
                    }
                }
            }
        }
    }

    None
}

/// Print a multi-line block per busy worktree showing the same body
/// sections `gw rm` uses (Active Claude session / Lockfile holder /
/// processes with cwd in this worktree), via the shared
/// `busy_messages::render_busy_block`. Skips the `--force` guidance —
/// `gw list` is read-only.
///
/// No-op when there are zero busy rows. The cwd scan is `OnceLock`-cached
/// for the process, so calling this after `get_worktree_status` adds no
/// extra scans.
fn print_busy_details(rows: &[WorktreeRow]) {
    let busy_rows: Vec<&WorktreeRow> = rows.iter().filter(|r| r.status == "busy").collect();
    if busy_rows.is_empty() {
        return;
    }

    for row in busy_rows {
        let (hard, soft) = crate::operations::busy::detect_busy_tiered(&row.path);
        // detect_busy_tiered may return empty if a process exited between
        // get_worktree_status and now. Skip silently — the table already
        // showed it as busy and the user can re-run.
        if hard.is_empty() && soft.is_empty() {
            continue;
        }
        let block =
            crate::operations::busy_messages::render_busy_block(&row.worktree_id, &hard, &soft);
        println!();
        // The block already ends with a trailing newline; print as-is.
        print!("{}", block);
    }
}

fn print_summary_footer(rows: &[WorktreeRow]) {
    // The first worktree is the primary repo checkout — exclude it from the
    // "feature worktree" count.
    let feature_count = if rows.len() > 1 { rows.len() - 1 } else { 0 };
    if feature_count == 0 {
        return;
    }

    let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for row in rows {
        *counts.entry(row.status.as_str()).or_insert(0) += 1;
    }

    let mut summary_parts = Vec::new();
    for &status_name in &[
        "clean", "modified", "busy", "active", "pr-open", "merged", "stale",
    ] {
        if let Some(&count) = counts.get(status_name) {
            if count > 0 {
                let styled = cwconsole::status_style(status_name)
                    .apply_to(format!("{} {}", count, status_name));
                summary_parts.push(styled.to_string());
            }
        }
    }

    let summary = if summary_parts.is_empty() {
        format!("\n{} feature worktree(s)", feature_count)
    } else {
        format!(
            "\n{} feature worktree(s) — {}",
            feature_count,
            summary_parts.join(", ")
        )
    };
    println!("{}", summary);
}

fn print_worktree_table(rows: &[WorktreeRow]) {
    let max_wt = rows.iter().map(|r| r.worktree_id.len()).max().unwrap_or(20);
    let max_br = rows
        .iter()
        .map(|r| r.current_branch.len())
        .max()
        .unwrap_or(20);
    let wt_col = max_wt.clamp(12, 35) + 2;
    let br_col = max_br.clamp(12, 35) + 2;

    println!(
        "  {} {:<wt_col$} {:<br_col$} {:<10} {:<12} {}",
        style(" ").dim(),
        style("WORKTREE").dim(),
        style("BRANCH").dim(),
        style("STATUS").dim(),
        style("AGE").dim(),
        style("PATH").dim(),
        wt_col = wt_col,
        br_col = br_col,
    );
    let line_width = (wt_col + br_col + 40).min(cwconsole::terminal_width().saturating_sub(4));
    println!("  {}", style("─".repeat(line_width)).dim());

    for row in rows {
        let icon = cwconsole::status_icon(&row.status);
        let st = cwconsole::status_style(&row.status);

        let branch_display = if row.worktree_id != row.current_branch {
            style(format!("{} ⚠", row.current_branch))
                .yellow()
                .to_string()
        } else {
            row.current_branch.clone()
        };

        let status_styled = st.apply_to(format!("{:<10}", row.status));

        println!(
            "  {} {:<wt_col$} {:<br_col$} {} {:<12} {}",
            st.apply_to(icon),
            style(&row.worktree_id).bold(),
            branch_display,
            status_styled,
            style(&row.age).dim(),
            style(&row.rel_path).dim(),
            wt_col = wt_col,
            br_col = br_col,
        );
    }
}

fn print_worktree_compact(rows: &[WorktreeRow]) {
    for row in rows {
        let icon = cwconsole::status_icon(&row.status);
        let st = cwconsole::status_style(&row.status);
        let age_part = if row.age.is_empty() {
            String::new()
        } else {
            format!("  {}", style(&row.age).dim())
        };

        println!(
            "  {} {}  {}{}",
            st.apply_to(icon),
            style(&row.worktree_id).bold(),
            st.apply_to(&row.status),
            age_part,
        );

        let mut details = Vec::new();
        if row.worktree_id != row.current_branch {
            details.push(format!(
                "branch: {}",
                style(format!("{} ⚠", row.current_branch)).yellow()
            ));
        }
        if !row.rel_path.is_empty() {
            details.push(format!("{}", style(&row.rel_path).dim()));
        }
        if !details.is_empty() {
            println!("      {}", details.join("  "));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_age_just_now() {
        assert_eq!(format_age(0.0), "just now");
        assert_eq!(format_age(0.001), "just now"); // ~1.4 minutes
    }

    #[test]
    fn test_format_age_hours() {
        assert_eq!(format_age(1.0 / 24.0), "1h ago"); // exactly 1 hour
        assert_eq!(format_age(0.5), "12h ago"); // 12 hours
        assert_eq!(format_age(0.99), "23h ago"); // ~23.7 hours
    }

    #[test]
    fn test_format_age_days() {
        assert_eq!(format_age(1.0), "1d ago");
        assert_eq!(format_age(1.5), "1d ago");
        assert_eq!(format_age(6.9), "6d ago");
    }

    #[test]
    fn test_format_age_weeks() {
        assert_eq!(format_age(7.0), "1w ago");
        assert_eq!(format_age(14.0), "2w ago");
        assert_eq!(format_age(29.0), "4w ago");
    }

    #[test]
    fn test_format_age_months() {
        assert_eq!(format_age(30.0), "1mo ago");
        assert_eq!(format_age(60.0), "2mo ago");
        assert_eq!(format_age(364.0), "12mo ago");
    }

    #[test]
    fn test_format_age_years() {
        assert_eq!(format_age(365.0), "1y ago");
        assert_eq!(format_age(730.0), "2y ago");
    }

    #[test]
    fn test_format_age_boundary_below_one_hour() {
        // Less than 1 hour (1/24 day ≈ 0.0417)
        assert_eq!(format_age(0.04), "just now"); // 0.04 * 24 = 0.96h → 0 as i64
    }

    #[test]
    fn format_selector_row_no_busy() {
        let row = format_selector_row("feat/a", "2d ago", false, "feat-a", 30);
        // branch (30) + space + age (9) + space + busy_pad (7) + path
        assert_eq!(
            row,
            "feat/a                         2d ago           feat-a"
        );
    }

    #[test]
    fn format_selector_row_busy_contains_badge() {
        let row = format_selector_row("fix/b", "3w ago", true, "fix-b", 30);
        assert!(
            row.contains("[busy]"),
            "expected [busy] in row, got: {:?}",
            row
        );
        assert!(row.contains("fix/b"));
        assert!(row.contains("3w ago"));
        assert!(row.contains("fix-b"));
    }

    #[test]
    fn format_selector_row_busy_visible_width_matches_no_busy() {
        // ANSI-colored [busy] must occupy the same visible width as 7 spaces,
        // so columns stay aligned under and not-under the cursor.
        let plain = format_selector_row("x", "1d ago", false, "p", 30);
        let busy = format_selector_row("x", "1d ago", true, "p", 30);
        assert_eq!(
            crate::tui::arrow_select::visible_len(&plain),
            crate::tui::arrow_select::visible_len(&busy),
        );
    }

    #[test]
    fn format_selector_row_empty_age_pads_to_nine() {
        let row = format_selector_row("feat/a", "", false, "feat-a", 30);
        // 30 branch + 1 sep + 9 age + 1 sep + 7 busy_pad = 48, then path starts.
        // Verify the path "feat-a" starts at byte 48.
        assert_eq!(&row[48..], "feat-a");
    }

    #[test]
    fn format_selector_row_long_branch_does_not_truncate() {
        let branch = "feat/extra-long-branch-name-well-past-thirty-chars";
        let row = format_selector_row(branch, "1d ago", false, "p", 30);
        assert!(
            row.starts_with(branch),
            "branch must not be truncated, got: {:?}",
            row
        );
    }

    // Note: this test exercises only the busy signal — repo/worktree
    // wiring (git::parse_worktrees etc.) is not exercised; the path is
    // used as a bare directory.
    #[test]
    #[cfg(unix)]
    fn test_get_worktree_status_busy_from_lockfile() {
        use crate::operations::lockfile::LockEntry;
        use std::fs;
        use std::process::{Command, Stdio};

        let tmp = tempfile::TempDir::new().unwrap();
        let repo = tmp.path();
        let wt = repo.join("wt1");
        fs::create_dir_all(wt.join(".git")).unwrap();

        // Spawn a child process: its PID is a descendant (not ancestor) of
        // the current process, so self_process_tree() will not contain it.
        // This gives us a live foreign PID to prove the busy signal fires.
        let mut child = Command::new("sleep")
            .arg("30")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn sleep");
        let foreign_pid: u32 = child.id();

        let entry = LockEntry {
            version: crate::operations::lockfile::LOCK_VERSION,
            pid: foreign_pid,
            started_at: 0,
            cmd: "claude".to_string(),
        };
        fs::write(
            wt.join(".git").join("gw-session.lock"),
            serde_json::to_string(&entry).unwrap(),
        )
        .unwrap();

        let status = get_worktree_status(&wt, repo, Some("wt1"), &PrCache::default());

        // Clean up child before asserting, so a failed assert still reaps it.
        let _ = child.kill();
        let _ = child.wait();

        assert_eq!(status, "busy");
    }

    /// Regression: `get_worktree_status` must not mark a worktree busy on
    /// the strength of a stale jsonl alone. Same scenario as the
    /// detect_busy_tiered regression — we plant a fresh-looking jsonl
    /// without any live claude process, and verify the worktree is NOT
    /// reported as busy.
    #[test]
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn test_get_worktree_status_not_busy_when_jsonl_active_but_no_live_claude() {
        use crate::operations::test_env::{env_lock, EnvGuard};
        let _lock = env_lock();
        let _guard = EnvGuard::capture(&["HOME"]);

        let home = tempfile::TempDir::new().unwrap();
        std::env::set_var("HOME", home.path());

        let repo = tempfile::TempDir::new().unwrap();
        let wt = repo.path().join("wt1");
        std::fs::create_dir_all(wt.join(".git")).unwrap();
        let wt_canon = wt.canonicalize().unwrap_or(wt.clone());

        // Plant a jsonl whose newest event is now (well within the 10-minute
        // threshold) and whose `cwd` matches the worktree.
        let encoded = wt_canon.to_string_lossy().replace(['/', '.'], "-");
        let proj_dir = home.path().join(".claude").join("projects").join(encoded);
        std::fs::create_dir_all(&proj_dir).unwrap();
        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let line = serde_json::json!({
            "timestamp": now,
            "cwd": wt_canon.to_string_lossy(),
        });
        std::fs::write(
            proj_dir.join("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee.jsonl"),
            format!("{}\n", line),
        )
        .unwrap();

        let status = get_worktree_status(&wt, repo.path(), Some("wt1"), &PrCache::default());
        assert_ne!(
            status, "busy",
            "expected non-busy without a live claude process, got busy"
        );
    }

    #[test]
    fn test_get_worktree_status_stale() {
        use std::path::PathBuf;
        let non_existent = PathBuf::from("/tmp/gw-test-nonexistent-12345");
        let repo = PathBuf::from("/tmp");
        assert_eq!(
            get_worktree_status(&non_existent, &repo, None, &PrCache::default()),
            "stale"
        );
    }
}
