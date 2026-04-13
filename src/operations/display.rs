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
    CONFIG_KEY_BASE_PATH, CONFIG_KEY_INTENDED_BRANCH,
};
use crate::error::Result;
use crate::git;

use rayon::prelude::*;

use super::pr_cache::PrCache;

/// Minimum terminal width for table layout; below this, use compact layout.
const MIN_TABLE_WIDTH: usize = 100;

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
///
/// TODO(perf): #24 — hoist `base_branch` lookup out of this function. Build a
/// `HashMap<branch, base>` in `list_worktrees` before the par_iter and pass it
/// in via a `WorktreeContext` struct alongside `pr_cache` and `cwd_canon`.
///
/// TODO(perf): #25 — hoist `cwd_canon` (`std::env::current_dir()?.canonicalize()`)
/// out of this function. Currently computed per worktree; one canonicalization
/// per `list_worktrees` call is sufficient. Pass via `WorktreeContext`.
//
// #22: WorktreeContext example kept as a plain comment so it doesn't appear in
// rustdoc. Future refactor: see plan doc for WorktreeContext design.
//
// pub struct WorktreeContext<'a> {
//     pub repo: &'a Path,
//     pub pr_cache: &'a PrCache,
//     pub base_branches: &'a HashMap<String, String>,
//     pub cwd_canon: Option<&'a Path>,
// }
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
    // detect_busy so the caller's own shell does not self-report.
    if !crate::operations::busy::detect_busy(path).is_empty() {
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
}

/// Serial-prep input passed to status computation.
/// Shares all fields with `WorktreeRow` except `path` (used to compute status)
/// and `status` itself (filled in by the parallel worker).
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
        }
    }
}

/// List all worktrees for the current repository.
pub fn list_worktrees(no_cache: bool) -> Result<()> {
    let repo = git::get_repo_root(None)?;
    let worktrees = git::parse_worktrees(&repo)?;

    println!(
        "\n{}  {}\n",
        style("Worktrees for repository:").cyan().bold(),
        repo.display()
    );

    let pr_cache = PrCache::load_or_fetch(&repo, no_cache);

    // Serial prep: cheap local work. Keep single-threaded for clarity.
    let inputs: Vec<RowInput> = worktrees
        .iter()
        .map(|(branch, path)| {
            let current_branch = git::normalize_branch_name(branch).to_string();
            let rel_path = pathdiff::diff_paths(path, &repo)
                .map(|p: std::path::PathBuf| p.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string_lossy().to_string());
            let age = path_age_str(path);
            let intended_branch = lookup_intended_branch(&repo, &current_branch, path);
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

    if inputs.is_empty() {
        println!("  {}\n", style("No worktrees found.").dim());
        return Ok(());
    }

    let is_tty = crate::tui::stdout_is_tty();
    // #18/#33/#35: cache terminal_width() once — used in both the progressive/static
    // branch decision and the post-render print guard.
    let term_width = cwconsole::terminal_width();
    // #35: extract narrow so the two places that check MIN_TABLE_WIDTH share
    // a single bool and cannot drift out of sync.
    let narrow = term_width < MIN_TABLE_WIDTH;
    let use_progressive = is_tty && !narrow;

    let rows: Vec<WorktreeRow> = if use_progressive {
        render_rows_progressive(&repo, &pr_cache, inputs)?
    } else {
        // rayon borrows &pr_cache across workers via the type system.
        inputs
            .into_par_iter()
            .map(|i| {
                let status =
                    get_worktree_status(&i.path, &repo, Some(&i.current_branch), &pr_cache);
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

    // +2 for header row and one trailing blank line.
    let viewport_height = u16::try_from(inputs.len())
        .unwrap_or(u16::MAX)
        .saturating_add(2)
        .max(3);

    let stdout = std::io::stdout();
    let backend = CrosstermBackend::new(stdout);
    // #1: mark active BEFORE construction so the panic hook fires correctly
    // if Terminal::with_options itself panics. If it returns Err, we clear
    // the flag before propagating so a non-ratatui panic later is not mishandled.
    crate::tui::mark_ratatui_active();
    let terminal = match Terminal::with_options(
        backend,
        TerminalOptions {
            viewport: Viewport::Inline(viewport_height),
        },
    ) {
        Ok(t) => t,
        Err(e) => {
            crate::tui::mark_ratatui_inactive();
            return Err(e.into());
        }
    };
    let mut guard = TerminalGuard::new(terminal);

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
    // #2: thread::scope blocks until all spawned threads finish; explicit
    // producer.join() collects panic state for diagnostics before `?` propagates.
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

    Ok(app.into_rows().into_iter().map(Into::into).collect())
}

/// Field-for-field 1:1 mapping from TUI row data to the internal display row.
/// Both structs are intentionally isomorphic; if `RowData` gains fields,
/// this impl must update.
impl From<crate::tui::list_view::RowData> for WorktreeRow {
    fn from(r: crate::tui::list_view::RowData) -> Self {
        WorktreeRow {
            worktree_id: r.worktree_id,
            current_branch: r.current_branch,
            status: r.status,
            age: r.age,
            rel_path: r.rel_path,
        }
    }
}

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

/// Show status of current worktree and list all worktrees.
pub fn show_status(no_cache: bool) -> Result<()> {
    let repo = git::get_repo_root(None)?;

    match git::get_current_branch(Some(&std::env::current_dir().unwrap_or_default())) {
        Ok(branch) => {
            let base_key = format_config_key(CONFIG_KEY_BASE_BRANCH, &branch);
            let path_key = format_config_key(CONFIG_KEY_BASE_PATH, &branch);
            let base = git::get_config(&base_key, Some(&repo));
            let base_path = git::get_config(&path_key, Some(&repo));

            println!("\n{}", style("Current worktree:").cyan().bold());
            println!("  Feature:  {}", style(&branch).green());
            println!(
                "  Base:     {}",
                style(base.as_deref().unwrap_or("N/A")).green()
            );
            println!(
                "  Base path: {}\n",
                style(base_path.as_deref().unwrap_or("N/A")).blue()
            );
        }
        Err(_) => {
            println!(
                "\n{}\n",
                style("Current directory is not a feature worktree or is the main repository.")
                    .yellow()
            );
        }
    }

    list_worktrees(no_cache)
}

/// Display worktree hierarchy in a visual tree format.
pub fn show_tree(no_cache: bool) -> Result<()> {
    let repo = git::get_repo_root(None)?;
    let cwd = std::env::current_dir().unwrap_or_default();

    let repo_name = repo
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "repo".to_string());

    println!(
        "\n{} (base repository)",
        style(format!("{}/", repo_name)).cyan().bold()
    );
    println!("{}\n", style(repo.display().to_string()).dim());

    let feature_worktrees = git::get_feature_worktrees(Some(&repo))?;

    if feature_worktrees.is_empty() {
        println!("{}\n", style("  (no feature worktrees)").dim());
        return Ok(());
    }

    let mut sorted = feature_worktrees;
    sorted.sort_by(|a, b| a.0.cmp(&b.0));

    let pr_cache = PrCache::load_or_fetch(&repo, no_cache);

    for (i, (branch_name, path)) in sorted.iter().enumerate() {
        let is_last = i == sorted.len() - 1;
        let prefix = if is_last { "└── " } else { "├── " };

        let status = get_worktree_status(path, &repo, Some(branch_name.as_str()), &pr_cache);
        let is_current = cwd
            .to_string_lossy()
            .starts_with(&path.to_string_lossy().to_string());

        let icon = cwconsole::status_icon(&status);
        let st = cwconsole::status_style(&status);

        let branch_display = if is_current {
            st.clone()
                .bold()
                .apply_to(format!("★ {}", branch_name))
                .to_string()
        } else {
            st.clone().apply_to(branch_name.as_str()).to_string()
        };

        let age = path_age_str(path);
        let age_display = if age.is_empty() {
            String::new()
        } else {
            format!("  {}", style(age).dim())
        };

        println!(
            "{}{} {}{}",
            prefix,
            st.apply_to(icon),
            branch_display,
            age_display
        );

        let path_display = if let Ok(rel) = path.strip_prefix(repo.parent().unwrap_or(&repo)) {
            format!("../{}", rel.display())
        } else {
            path.display().to_string()
        };

        let continuation = if is_last { "    " } else { "│   " };
        println!("{}{}", continuation, style(&path_display).dim());
    }

    // Legend
    println!("\n{}", style("Legend:").bold());
    println!(
        "  {} active (current)",
        cwconsole::status_style("active").apply_to("●")
    );
    println!("  {} clean", cwconsole::status_style("clean").apply_to("○"));
    println!(
        "  {} modified",
        cwconsole::status_style("modified").apply_to("◉")
    );
    println!(
        "  {} pr-open",
        cwconsole::status_style("pr-open").apply_to("⬆")
    );
    println!(
        "  {} merged",
        cwconsole::status_style("merged").apply_to("✓")
    );
    println!(
        "  {} busy (other session)",
        cwconsole::status_style("busy").apply_to("🔒")
    );
    println!("  {} stale", cwconsole::status_style("stale").apply_to("x"));
    println!(
        "  {} currently active worktree\n",
        style("★").green().bold()
    );

    Ok(())
}

/// Display usage analytics for worktrees.
pub fn show_stats(no_cache: bool) -> Result<()> {
    let repo = git::get_repo_root(None)?;
    let feature_worktrees = git::get_feature_worktrees(Some(&repo))?;

    if feature_worktrees.is_empty() {
        println!("\n{}\n", style("No feature worktrees found").yellow());
        return Ok(());
    }

    println!();
    println!("  {}", style("Worktree Statistics").cyan().bold());
    println!("  {}", style("─".repeat(40)).dim());
    println!();

    struct WtData {
        branch: String,
        status: String,
        age_days: f64,
        commit_count: usize,
    }

    let mut data: Vec<WtData> = Vec::new();

    let pr_cache = PrCache::load_or_fetch(&repo, no_cache);

    for (branch_name, path) in &feature_worktrees {
        let status = get_worktree_status(path, &repo, Some(branch_name.as_str()), &pr_cache);
        let age_days = path_age_days(path).unwrap_or(0.0);

        let commit_count = git::git_command(
            &["rev-list", "--count", branch_name],
            Some(path),
            false,
            true,
        )
        .ok()
        .and_then(|r| {
            if r.returncode == 0 {
                r.stdout.trim().parse::<usize>().ok()
            } else {
                None
            }
        })
        .unwrap_or(0);

        data.push(WtData {
            branch: branch_name.clone(),
            status,
            age_days,
            commit_count,
        });
    }

    // Overview
    let mut status_counts: std::collections::HashMap<&str, usize> =
        std::collections::HashMap::new();
    for d in &data {
        *status_counts.entry(d.status.as_str()).or_insert(0) += 1;
    }

    println!("  {} {}", style("Total:").bold(), data.len());

    // Status bar visualization
    let total = data.len();
    let bar_width = 30;
    let clean = *status_counts.get("clean").unwrap_or(&0);
    let modified = *status_counts.get("modified").unwrap_or(&0);
    let active = *status_counts.get("active").unwrap_or(&0);
    let pr_open = *status_counts.get("pr-open").unwrap_or(&0);
    let merged = *status_counts.get("merged").unwrap_or(&0);
    let busy = *status_counts.get("busy").unwrap_or(&0);
    let stale = *status_counts.get("stale").unwrap_or(&0);

    let bar_clean = (clean * bar_width) / total.max(1);
    let bar_modified = (modified * bar_width) / total.max(1);
    let bar_active = (active * bar_width) / total.max(1);
    let bar_pr_open = (pr_open * bar_width) / total.max(1);
    let bar_merged = (merged * bar_width) / total.max(1);
    let bar_busy = (busy * bar_width) / total.max(1);
    let bar_stale = (stale * bar_width) / total.max(1);
    // Fill remaining with clean if rounding left gaps
    let bar_remainder = bar_width
        - bar_clean
        - bar_modified
        - bar_active
        - bar_pr_open
        - bar_merged
        - bar_busy
        - bar_stale;

    print!("  ");
    print!("{}", style("█".repeat(bar_clean + bar_remainder)).green());
    print!("{}", style("█".repeat(bar_modified)).yellow());
    print!("{}", style("█".repeat(bar_active)).green().bold());
    print!("{}", style("█".repeat(bar_pr_open)).cyan());
    print!("{}", style("█".repeat(bar_merged)).magenta());
    print!("{}", style("█".repeat(bar_busy)).red().bold());
    print!("{}", style("█".repeat(bar_stale)).red());
    println!();

    let mut parts = Vec::new();
    if clean > 0 {
        parts.push(format!("{}", style(format!("○ {} clean", clean)).green()));
    }
    if modified > 0 {
        parts.push(format!(
            "{}",
            style(format!("◉ {} modified", modified)).yellow()
        ));
    }
    if active > 0 {
        parts.push(format!(
            "{}",
            style(format!("● {} active", active)).green().bold()
        ));
    }
    if pr_open > 0 {
        parts.push(format!(
            "{}",
            style(format!("⬆ {} pr-open", pr_open)).cyan()
        ));
    }
    if merged > 0 {
        parts.push(format!(
            "{}",
            style(format!("✓ {} merged", merged)).magenta()
        ));
    }
    if busy > 0 {
        parts.push(format!(
            "{}",
            style(format!("🔒 {} busy", busy)).red().bold()
        ));
    }
    if stale > 0 {
        parts.push(format!("{}", style(format!("x {} stale", stale)).red()));
    }
    println!("  {}", parts.join("  "));
    println!();

    // Age statistics
    let ages: Vec<f64> = data
        .iter()
        .filter(|d| d.age_days > 0.0)
        .map(|d| d.age_days)
        .collect();
    if !ages.is_empty() {
        let avg = ages.iter().sum::<f64>() / ages.len() as f64;
        let oldest = ages.iter().cloned().fold(0.0_f64, f64::max);
        let newest = ages.iter().cloned().fold(f64::MAX, f64::min);

        println!("  {} Age", style("◷").dim());
        println!(
            "    avg {}  oldest {}  newest {}",
            style(format!("{:.1}d", avg)).bold(),
            style(format!("{:.1}d", oldest)).yellow(),
            style(format!("{:.1}d", newest)).green(),
        );
        println!();
    }

    // Commit statistics
    let commits: Vec<usize> = data
        .iter()
        .filter(|d| d.commit_count > 0)
        .map(|d| d.commit_count)
        .collect();
    if !commits.is_empty() {
        let total: usize = commits.iter().sum();
        let avg = total as f64 / commits.len() as f64;
        let max_c = *commits.iter().max().unwrap_or(&0);

        println!("  {} Commits", style("⟲").dim());
        println!(
            "    total {}  avg {:.1}  max {}",
            style(total).bold(),
            avg,
            style(max_c).bold(),
        );
        println!();
    }

    // Top by age
    println!("  {}", style("Oldest Worktrees").bold());
    let mut by_age = data.iter().collect::<Vec<_>>();
    by_age.sort_by(|a, b| b.age_days.total_cmp(&a.age_days));
    let max_age = by_age.first().map(|d| d.age_days).unwrap_or(1.0).max(1.0);
    for d in by_age.iter().take(5) {
        if d.age_days > 0.0 {
            let icon = cwconsole::status_icon(&d.status);
            let st = cwconsole::status_style(&d.status);
            let bar_len = ((d.age_days / max_age) * 15.0) as usize;
            println!(
                "    {} {:<25} {} {}",
                st.apply_to(icon),
                d.branch,
                style("▓".repeat(bar_len.max(1))).dim(),
                style(format_age(d.age_days)).dim(),
            );
        }
    }
    println!();

    // Top by commits
    println!("  {}", style("Most Active (by commits)").bold());
    let mut by_commits = data.iter().collect::<Vec<_>>();
    by_commits.sort_by(|a, b| b.commit_count.cmp(&a.commit_count));
    let max_commits = by_commits
        .first()
        .map(|d| d.commit_count)
        .unwrap_or(1)
        .max(1);
    for d in by_commits.iter().take(5) {
        if d.commit_count > 0 {
            let icon = cwconsole::status_icon(&d.status);
            let st = cwconsole::status_style(&d.status);
            let bar_len = (d.commit_count * 15) / max_commits;
            println!(
                "    {} {:<25} {} {}",
                st.apply_to(icon),
                d.branch,
                style("▓".repeat(bar_len.max(1))).cyan(),
                style(format!("{} commits", d.commit_count)).dim(),
            );
        }
    }
    println!();

    Ok(())
}

/// Compare two branches.
pub fn diff_worktrees(branch1: &str, branch2: &str, summary: bool, files: bool) -> Result<()> {
    let repo = git::get_repo_root(None)?;

    if !git::branch_exists(branch1, Some(&repo)) {
        return Err(crate::error::CwError::InvalidBranch(format!(
            "Branch '{}' not found",
            branch1
        )));
    }
    if !git::branch_exists(branch2, Some(&repo)) {
        return Err(crate::error::CwError::InvalidBranch(format!(
            "Branch '{}' not found",
            branch2
        )));
    }

    println!("\n{}", style("Comparing branches:").cyan().bold());
    println!("  {} {} {}\n", branch1, style("...").yellow(), branch2);

    if files {
        let result = git::git_command(
            &["diff", "--name-status", branch1, branch2],
            Some(&repo),
            true,
            true,
        )?;
        println!("{}\n", style("Changed files:").bold());
        if result.stdout.trim().is_empty() {
            println!("  {}", style("No differences found").dim());
        } else {
            for line in result.stdout.trim().lines() {
                let parts: Vec<&str> = line.splitn(2, char::is_whitespace).collect();
                if parts.len() == 2 {
                    let (status_char, filename) = (parts[0], parts[1]);
                    let c = status_char.chars().next().unwrap_or('?');
                    let status_name = match c {
                        'M' => "Modified",
                        'A' => "Added",
                        'D' => "Deleted",
                        'R' => "Renamed",
                        'C' => "Copied",
                        _ => "Changed",
                    };
                    let styled_status = match c {
                        'M' => style(status_char).yellow(),
                        'A' => style(status_char).green(),
                        'D' => style(status_char).red(),
                        'R' | 'C' => style(status_char).cyan(),
                        _ => style(status_char),
                    };
                    println!("  {}  {} ({})", styled_status, filename, status_name);
                }
            }
        }
    } else if summary {
        let result = git::git_command(
            &["diff", "--stat", branch1, branch2],
            Some(&repo),
            true,
            true,
        )?;
        println!("{}\n", style("Diff summary:").bold());
        if result.stdout.trim().is_empty() {
            println!("  {}", style("No differences found").dim());
        } else {
            println!("{}", result.stdout);
        }
    } else {
        let result = git::git_command(&["diff", branch1, branch2], Some(&repo), true, true)?;
        if result.stdout.trim().is_empty() {
            println!("{}\n", style("No differences found").dim());
        } else {
            println!("{}", result.stdout);
        }
    }

    Ok(())
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
