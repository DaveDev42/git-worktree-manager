use std::process::Command;

use console::style;

use crate::constants::{format_config_key, CONFIG_KEY_BASE_BRANCH};
use crate::error::Result;
use crate::git;

use super::display::get_worktree_status;

/// Perform health check on all worktrees.
pub fn doctor() -> Result<()> {
    let repo = git::get_repo_root(None)?;
    println!(
        "\n{}\n",
        style("claude-worktree Health Check").cyan().bold()
    );

    let mut issues = 0u32;
    let mut warnings = 0u32;

    // 1. Check Git version
    println!("{}", style("1. Checking Git version...").bold());
    match Command::new("git").arg("--version").output() {
        Ok(output) if output.status.success() => {
            let version_output = String::from_utf8_lossy(&output.stdout);
            let version_str = version_output
                .split_whitespace()
                .nth(2)
                .unwrap_or("unknown");

            let parts: Vec<u32> = version_str
                .split('.')
                .filter_map(|p| p.parse().ok())
                .collect();
            let is_ok = parts.len() >= 2 && (parts[0] > 2 || (parts[0] == 2 && parts[1] >= 31));

            if is_ok {
                println!(
                    "   {} Git version {} (minimum: 2.31.0)",
                    style("*").green(),
                    version_str
                );
            } else {
                println!(
                    "   {} Git version {} is too old (minimum: 2.31.0)",
                    style("x").red(),
                    version_str
                );
                issues += 1;
            }
        }
        _ => {
            println!("   {} Could not detect Git version", style("x").red());
            issues += 1;
        }
    }
    println!();

    // 2. Check worktree accessibility
    println!("{}", style("2. Checking worktree accessibility...").bold());
    let feature_worktrees = git::get_feature_worktrees(Some(&repo))?;
    let mut stale_count = 0u32;

    struct WtInfo {
        branch: String,
        path: std::path::PathBuf,
        status: String,
    }

    let mut worktrees: Vec<WtInfo> = Vec::new();

    for (branch_name, path) in &feature_worktrees {
        let status = get_worktree_status(path, &repo);
        if status == "stale" {
            stale_count += 1;
            println!(
                "   {} {}: Stale (directory missing)",
                style("x").red(),
                branch_name
            );
            issues += 1;
        }
        worktrees.push(WtInfo {
            branch: branch_name.clone(),
            path: path.clone(),
            status,
        });
    }

    if stale_count == 0 {
        println!(
            "   {} All {} worktrees are accessible",
            style("*").green(),
            worktrees.len()
        );
    }
    println!();

    // 3. Check for uncommitted changes
    println!("{}", style("3. Checking for uncommitted changes...").bold());
    let mut dirty: Vec<String> = Vec::new();
    for wt in &worktrees {
        if wt.status == "modified" || wt.status == "active" {
            if let Ok(r) = git::git_command(&["status", "--porcelain"], Some(&wt.path), false, true)
            {
                if r.returncode == 0 && !r.stdout.trim().is_empty() {
                    dirty.push(wt.branch.clone());
                }
            }
        }
    }

    if dirty.is_empty() {
        println!("   {} No uncommitted changes", style("*").green());
    } else {
        println!(
            "   {} {} worktree(s) with uncommitted changes:",
            style("!").yellow(),
            dirty.len()
        );
        for b in &dirty {
            println!("      - {}", b);
        }
        warnings += 1;
    }
    println!();

    // 4. Check if worktrees are behind base branch
    println!(
        "{}",
        style("4. Checking if worktrees are behind base branch...").bold()
    );
    let mut behind: Vec<(String, String, String)> = Vec::new();

    for wt in &worktrees {
        if wt.status == "stale" {
            continue;
        }
        let key = format_config_key(CONFIG_KEY_BASE_BRANCH, &wt.branch);
        let base = match git::get_config(&key, Some(&repo)) {
            Some(b) => b,
            None => continue,
        };

        let origin_base = format!("origin/{}", base);
        if let Ok(r) = git::git_command(
            &[
                "rev-list",
                "--count",
                &format!("{}..{}", wt.branch, origin_base),
            ],
            Some(&wt.path),
            false,
            true,
        ) {
            if r.returncode == 0 {
                let count = r.stdout.trim();
                if count != "0" {
                    behind.push((wt.branch.clone(), base.clone(), count.to_string()));
                }
            }
        }
    }

    if behind.is_empty() {
        println!(
            "   {} All worktrees are up-to-date with base",
            style("*").green()
        );
    } else {
        println!(
            "   {} {} worktree(s) behind base branch:",
            style("!").yellow(),
            behind.len()
        );
        for (b, base, count) in &behind {
            println!("      - {}: {} commit(s) behind {}", b, count, base);
        }
        println!(
            "   {}",
            style("Tip: Use 'cw sync --all' to update all worktrees").dim()
        );
        warnings += 1;
    }
    println!();

    // 5. Check for merge conflicts
    println!("{}", style("5. Checking for merge conflicts...").bold());
    let mut conflicted: Vec<(String, usize)> = Vec::new();

    for wt in &worktrees {
        if wt.status == "stale" {
            continue;
        }
        if let Ok(r) = git::git_command(
            &["diff", "--name-only", "--diff-filter=U"],
            Some(&wt.path),
            false,
            true,
        ) {
            if r.returncode == 0 && !r.stdout.trim().is_empty() {
                let count = r.stdout.trim().lines().count();
                conflicted.push((wt.branch.clone(), count));
            }
        }
    }

    if conflicted.is_empty() {
        println!("   {} No merge conflicts detected", style("*").green());
    } else {
        println!(
            "   {} {} worktree(s) with merge conflicts:",
            style("x").red(),
            conflicted.len()
        );
        for (b, count) in &conflicted {
            println!("      - {}: {} conflicted file(s)", b, count);
        }
        issues += 1;
    }
    println!();

    // Summary
    println!("{}", style("Summary:").cyan().bold());
    if issues == 0 && warnings == 0 {
        println!("{}\n", style("* Everything looks healthy!").green().bold());
    } else {
        if issues > 0 {
            println!(
                "{}",
                style(format!("x {} issue(s) found", issues)).red().bold()
            );
        }
        if warnings > 0 {
            println!(
                "{}",
                style(format!("! {} warning(s) found", warnings))
                    .yellow()
                    .bold()
            );
        }
        println!();
    }

    // Recommendations
    let has_recommendations = stale_count > 0 || !behind.is_empty() || !conflicted.is_empty();
    if has_recommendations {
        println!("{}", style("Recommendations:").bold());
        if stale_count > 0 {
            println!(
                "  - Run {} to clean up stale worktrees",
                style("cw prune").cyan()
            );
        }
        if !behind.is_empty() {
            println!(
                "  - Run {} to update all worktrees",
                style("cw sync --all").cyan()
            );
        }
        if !conflicted.is_empty() {
            println!("  - Resolve conflicts in conflicted worktrees");
        }
        println!();
    }

    Ok(())
}
