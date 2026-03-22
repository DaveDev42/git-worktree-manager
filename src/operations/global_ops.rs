//! Global worktree management operations.
//!
//! Mirrors src/git_worktree_manager/operations/global_ops.py (237 lines).
//! Business logic for cross-repository worktree commands (`gw -g`).

use console::style;

use crate::console as cwconsole;
use crate::constants::{format_config_key, home_dir_or_fallback, path_age_days, CONFIG_KEY_INTENDED_BRANCH};
use crate::error::Result;
use crate::git;
use crate::registry;

use super::display::{format_age, get_worktree_status};

/// Collected row for global worktree display.
struct GlobalWorktreeRow {
    repo_name: String,
    worktree_id: String,
    current_branch: String,
    status: String,
    age: String,
    rel_path: String,
}

/// Minimum terminal width for global table layout (wider than local due to REPO column).
const MIN_GLOBAL_TABLE_WIDTH: usize = 125;

/// List worktrees across all registered repositories.
pub fn global_list_worktrees() -> Result<()> {
    // Auto-prune stale entries before listing
    if let Ok(removed) = registry::prune_registry() {
        if !removed.is_empty() {
            println!(
                "{}",
                style(format!(
                    "Auto-pruned {} stale registry entry(s)",
                    removed.len()
                ))
                .dim()
            );
        }
    }

    let repos = registry::get_all_registered_repos();

    if repos.is_empty() {
        println!(
            "\n{}\n\
             Use {} to discover repositories,\n\
             or run {} in a repository to auto-register it.\n",
            style("No repositories registered.").yellow(),
            style("gw -g scan").cyan(),
            style("gw new").cyan(),
        );
        return Ok(());
    }

    println!("\n{}\n", style("Global Worktree Overview").cyan().bold());

    let mut total_repos = 0usize;
    let mut status_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut rows: Vec<GlobalWorktreeRow> = Vec::new();

    let mut sorted_repos = repos;
    sorted_repos.sort_by(|a, b| a.0.cmp(&b.0));

    for (name, repo_path) in &sorted_repos {
        if !repo_path.exists() {
            println!(
                "{} {} — {}",
                style(format!("⚠ {}", name)).yellow(),
                style(format!("({})", repo_path.display())).dim(),
                style("repository not found").red(),
            );
            continue;
        }

        let feature_wts = match git::get_feature_worktrees(Some(repo_path)) {
            Ok(wts) => wts,
            Err(_) => {
                println!(
                    "{} {} — {}",
                    style(format!("⚠ {}", name)).yellow(),
                    style(format!("({})", repo_path.display())).dim(),
                    style("failed to read worktrees").red(),
                );
                continue;
            }
        };

        let mut has_feature = false;
        for (branch_name, path) in &feature_wts {
            let status = get_worktree_status(path, repo_path);

            // Check intended branch for mismatch detection
            let intended_key = format_config_key(CONFIG_KEY_INTENDED_BRANCH, branch_name);
            let worktree_id =
                git::get_config(&intended_key, Some(repo_path)).unwrap_or(branch_name.clone());

            // Compute age
            let age = path_age_days(path)
                .map(format_age)
                .unwrap_or_default();

            // Relative path
            let rel_path = pathdiff::diff_paths(path, repo_path)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string_lossy().to_string());

            *status_counts.entry(status.clone()).or_insert(0) += 1;

            rows.push(GlobalWorktreeRow {
                repo_name: name.clone(),
                worktree_id,
                current_branch: branch_name.clone(),
                status,
                age,
                rel_path,
            });

            has_feature = true;
        }

        if has_feature {
            total_repos += 1;
        }
    }

    if rows.is_empty() {
        println!(
            "{}\n",
            style("No repositories with active worktrees found.").yellow()
        );
        return Ok(());
    }

    // Choose layout based on terminal width
    let term_width = cwconsole::terminal_width();
    if term_width >= MIN_GLOBAL_TABLE_WIDTH {
        global_print_table(&rows);
    } else {
        global_print_compact(&rows);
    }

    // Summary footer
    let total_worktrees = rows.len();
    let mut summary_parts = Vec::new();
    for &status_name in &["clean", "modified", "active", "stale"] {
        if let Some(&count) = status_counts.get(status_name) {
            if count > 0 {
                let styled = cwconsole::status_style(status_name)
                    .apply_to(format!("{} {}", count, status_name));
                summary_parts.push(styled.to_string());
            }
        }
    }

    let summary = if summary_parts.is_empty() {
        format!("\n{} repo(s), {} worktree(s)", total_repos, total_worktrees)
    } else {
        format!(
            "\n{} repo(s), {} worktree(s) — {}",
            total_repos,
            total_worktrees,
            summary_parts.join(", ")
        )
    };
    println!("{}", summary);
    println!();

    Ok(())
}

fn global_print_table(rows: &[GlobalWorktreeRow]) {
    let max_repo = rows.iter().map(|r| r.repo_name.len()).max().unwrap_or(12);
    let max_wt = rows.iter().map(|r| r.worktree_id.len()).max().unwrap_or(20);
    let max_br = rows
        .iter()
        .map(|r| r.current_branch.len())
        .max()
        .unwrap_or(20);

    let repo_col = max_repo.clamp(12, 25) + 2;
    let wt_col = max_wt.clamp(20, 35) + 2;
    let br_col = max_br.clamp(20, 35) + 2;

    println!(
        "{:<repo_col$} {:<wt_col$} {:<br_col$} {:<10} {:<12} PATH",
        "REPO",
        "WORKTREE",
        "CURRENT BRANCH",
        "STATUS",
        "AGE",
        repo_col = repo_col,
        wt_col = wt_col,
        br_col = br_col,
    );
    println!("{}", "─".repeat(repo_col + wt_col + br_col + 82));

    for row in rows {
        let branch_display = if row.worktree_id != row.current_branch {
            style(format!("{} (⚠️)", row.current_branch))
                .yellow()
                .to_string()
        } else {
            row.current_branch.clone()
        };

        let status_styled =
            cwconsole::status_style(&row.status).apply_to(format!("{:<10}", row.status));

        println!(
            "{:<repo_col$} {:<wt_col$} {:<br_col$} {} {:<12} {}",
            row.repo_name,
            row.worktree_id,
            branch_display,
            status_styled,
            row.age,
            row.rel_path,
            repo_col = repo_col,
            wt_col = wt_col,
            br_col = br_col,
        );
    }
}

fn global_print_compact(rows: &[GlobalWorktreeRow]) {
    let mut current_repo = String::new();

    for row in rows {
        if row.repo_name != current_repo {
            if !current_repo.is_empty() {
                println!(); // blank line between repos
            }
            println!("{}", style(&row.repo_name).bold());
            current_repo = row.repo_name.clone();
        }

        let status_styled = cwconsole::status_style(&row.status).apply_to(&row.status);
        let age_part = if row.age.is_empty() {
            String::new()
        } else {
            format!("  {}", row.age)
        };

        println!(
            "  {}  {}{}",
            style(&row.worktree_id).bold(),
            status_styled,
            age_part,
        );

        let mut details = Vec::new();
        if row.worktree_id != row.current_branch {
            details.push(format!(
                "branch: {}",
                style(format!("{} (⚠️)", row.current_branch)).yellow()
            ));
        }
        details.push(format!("path: {}", row.rel_path));
        println!("    {}", details.join(" · "));
    }
}

/// Scan for repositories (improved format matching Python).
pub fn global_scan(base_dir: Option<&std::path::Path>) -> Result<()> {
    let scan_dir = base_dir
        .map(|p| p.to_path_buf())
        .unwrap_or_else(home_dir_or_fallback);

    println!(
        "\n{}\n  Directory: {}\n",
        style("Scanning for repositories...").cyan().bold(),
        style(scan_dir.display()).blue(),
    );

    let found = registry::scan_for_repos(base_dir, 5);

    if found.is_empty() {
        println!(
            "{}\n",
            style("No repositories with worktrees found.").yellow()
        );
        return Ok(());
    }

    println!(
        "{} Found {} repository(s):\n",
        style("*").green().bold(),
        found.len()
    );

    let mut sorted = found;
    sorted.sort();
    for repo_path in &sorted {
        let _ = registry::register_repo(repo_path);
        println!(
            "  {} {} {}",
            style("+").green(),
            repo_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
            style(format!("({})", repo_path.display())).dim(),
        );
    }

    println!(
        "\n{} Registered {} repository(s)\n\
         Use {} to see all worktrees.\n",
        style("*").green().bold(),
        sorted.len(),
        style("gw -g list").cyan(),
    );

    Ok(())
}

/// Remove stale entries from the global registry (improved format).
pub fn global_prune() -> Result<()> {
    println!("\n{}\n", style("Pruning registry...").cyan().bold());

    match registry::prune_registry() {
        Ok(removed) => {
            if removed.is_empty() {
                println!(
                    "{} Registry is clean, nothing to prune.\n",
                    style("*").green().bold()
                );
            } else {
                println!(
                    "{} Removed {} stale entry(s):\n",
                    style("*").green().bold(),
                    removed.len()
                );
                for path in &removed {
                    println!("  {} {}", style("-").red(), path);
                }
                println!();
            }
            Ok(())
        }
        Err(e) => Err(e),
    }
}
