/// Batch cleanup of worktrees.
///
/// Mirrors clean_worktrees from worktree_ops.py.
use std::time::SystemTime;

use console::style;

use crate::constants::{format_config_key, CONFIG_KEY_BASE_BRANCH};
use crate::error::Result;
use crate::git;

use super::display::get_worktree_status;

/// Batch cleanup of worktrees based on criteria.
pub fn clean_worktrees(
    merged: bool,
    older_than: Option<u64>,
    interactive: bool,
    dry_run: bool,
) -> Result<()> {
    let repo = git::get_repo_root(None)?;

    // Must specify at least one criterion
    if !merged && older_than.is_none() && !interactive {
        eprintln!(
            "Error: Please specify at least one cleanup criterion:\n  \
             --merged, --older-than, or -i/--interactive"
        );
        return Ok(());
    }

    let mut to_delete: Vec<(String, String, String)> = Vec::new(); // (branch, path, reason)

    for (branch_name, path) in git::get_feature_worktrees(Some(&repo))? {
        let mut should_delete = false;
        let mut reasons = Vec::new();

        // Check if merged
        if merged {
            let base_key = format_config_key(CONFIG_KEY_BASE_BRANCH, &branch_name);
            if let Some(base_branch) = git::get_config(&base_key, Some(&repo)) {
                if let Ok(r) = git::git_command(
                    &[
                        "branch",
                        "--merged",
                        &base_branch,
                        "--format=%(refname:short)",
                    ],
                    Some(&repo),
                    false,
                    true,
                ) {
                    if r.returncode == 0 && r.stdout.lines().any(|l| l.trim() == branch_name) {
                        should_delete = true;
                        reasons.push(format!("merged into {}", base_branch));
                    }
                }
            }
        }

        // Check age
        if let Some(days) = older_than {
            if path.exists() {
                if let Ok(meta) = path.metadata() {
                    if let Ok(modified) = meta.modified() {
                        if let Ok(age) = SystemTime::now().duration_since(modified) {
                            let age_days = age.as_secs() / 86400;
                            if age_days > days {
                                should_delete = true;
                                reasons
                                    .push(format!("older than {} days ({} days)", days, age_days));
                            }
                        }
                    }
                }
            }
        }

        if should_delete {
            to_delete.push((
                branch_name.clone(),
                path.to_string_lossy().to_string(),
                reasons.join(", "),
            ));
        }
    }

    // Interactive mode
    if interactive && to_delete.is_empty() {
        println!("{}\n", style("Available worktrees:").cyan().bold());
        let mut all_wt = Vec::new();
        for (branch_name, path) in git::get_feature_worktrees(Some(&repo))? {
            let status = get_worktree_status(&path, &repo);
            println!("  [{:8}] {:<30} {}", status, branch_name, path.display());
            all_wt.push((branch_name, path.to_string_lossy().to_string()));
        }
        println!();
        println!("Enter branch names to delete (space-separated), or 'all' for all:");

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.eq_ignore_ascii_case("all") {
            to_delete = all_wt
                .into_iter()
                .map(|(b, p)| (b, p, "user selected".to_string()))
                .collect();
        } else {
            let selected: Vec<&str> = input.split_whitespace().collect();
            to_delete = all_wt
                .into_iter()
                .filter(|(b, _)| selected.contains(&b.as_str()))
                .map(|(b, p)| (b, p, "user selected".to_string()))
                .collect();
        }

        if to_delete.is_empty() {
            println!("{}", style("No worktrees selected for deletion").yellow());
            return Ok(());
        }
    }

    if to_delete.is_empty() {
        println!(
            "{} No worktrees match the cleanup criteria\n",
            style("*").green().bold()
        );
        return Ok(());
    }

    // Show what will be deleted
    let prefix = if dry_run { "DRY RUN: " } else { "" };
    println!(
        "\n{}\n",
        style(format!("{}Worktrees to delete:", prefix))
            .yellow()
            .bold()
    );
    for (branch, path, reason) in &to_delete {
        println!("  - {:<30} ({})", branch, reason);
        println!("    Path: {}", path);
    }
    println!();

    if dry_run {
        println!(
            "{} Would delete {} worktree(s)",
            style("*").cyan().bold(),
            to_delete.len()
        );
        println!("Run without --dry-run to actually delete them");
        return Ok(());
    }

    // Delete worktrees
    let mut deleted = 0u32;
    for (branch, _, _) in &to_delete {
        println!("{}", style(format!("Deleting {}...", branch)).yellow());
        match super::worktree::delete_worktree(Some(branch), false, false) {
            Ok(()) => {
                println!("{} Deleted {}", style("*").green().bold(), branch);
                deleted += 1;
            }
            Err(e) => {
                println!(
                    "{} Failed to delete {}: {}",
                    style("x").red().bold(),
                    branch,
                    e
                );
            }
        }
    }

    println!(
        "\n{}\n",
        style(format!(
            "* Cleanup complete! Deleted {} worktree(s)",
            deleted
        ))
        .green()
        .bold()
    );

    // Prune stale metadata
    println!("{}", style("Pruning stale worktree metadata...").dim());
    let _ = git::git_command(&["worktree", "prune"], Some(&repo), false, false);
    println!("{}\n", style("* Prune complete").dim());

    Ok(())
}
