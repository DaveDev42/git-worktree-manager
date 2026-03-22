use std::process::Command;

use console::style;

use crate::constants::{
    format_config_key, version_meets_minimum, CONFIG_KEY_BASE_BRANCH, MIN_GIT_VERSION,
    MIN_GIT_VERSION_MAJOR, MIN_GIT_VERSION_MINOR,
};
use crate::error::Result;
use crate::git;

use super::display::get_worktree_status;

/// Worktree info collected during health check.
struct WtInfo {
    branch: String,
    path: std::path::PathBuf,
    status: String,
}

/// Perform health check on all worktrees.
pub fn doctor() -> Result<()> {
    let repo = git::get_repo_root(None)?;
    println!(
        "\n{}\n",
        style("git-worktree-manager Health Check").cyan().bold()
    );

    let mut issues = 0u32;
    let mut warnings = 0u32;

    // 1. Check Git version
    check_git_version(&mut issues);

    // 2. Check worktree accessibility
    let (worktrees, stale_count) = check_worktree_accessibility(&repo, &mut issues)?;

    // 3. Check for uncommitted changes
    check_uncommitted_changes(&worktrees, &mut warnings);

    // 4. Check if worktrees are behind base branch
    let behind = check_behind_base(&worktrees, &repo, &mut warnings);

    // 5. Check for merge conflicts
    let conflicted = check_merge_conflicts(&worktrees, &mut issues);

    // Summary
    print_summary(issues, warnings);

    // Recommendations
    print_recommendations(stale_count, &behind, &conflicted);

    Ok(())
}

/// Check Git version meets minimum requirement.
fn check_git_version(issues: &mut u32) {
    println!("{}", style("1. Checking Git version...").bold());
    match Command::new("git").arg("--version").output() {
        Ok(output) if output.status.success() => {
            let version_output = String::from_utf8_lossy(&output.stdout);
            let version_str = version_output
                .split_whitespace()
                .nth(2)
                .unwrap_or("unknown");

            let is_ok =
                version_meets_minimum(version_str, MIN_GIT_VERSION_MAJOR, MIN_GIT_VERSION_MINOR);

            if is_ok {
                println!(
                    "   {} Git version {} (minimum: {})",
                    style("*").green(),
                    version_str,
                    MIN_GIT_VERSION,
                );
            } else {
                println!(
                    "   {} Git version {} is too old (minimum: {})",
                    style("x").red(),
                    version_str,
                    MIN_GIT_VERSION,
                );
                *issues += 1;
            }
        }
        _ => {
            println!("   {} Could not detect Git version", style("x").red());
            *issues += 1;
        }
    }
    println!();
}

/// Check that all worktrees are accessible (not stale).
fn check_worktree_accessibility(
    repo: &std::path::Path,
    issues: &mut u32,
) -> Result<(Vec<WtInfo>, u32)> {
    println!("{}", style("2. Checking worktree accessibility...").bold());
    let feature_worktrees = git::get_feature_worktrees(Some(repo))?;
    let mut stale_count = 0u32;
    let mut worktrees: Vec<WtInfo> = Vec::new();

    for (branch_name, path) in &feature_worktrees {
        let status = get_worktree_status(path, repo);
        if status == "stale" {
            stale_count += 1;
            println!(
                "   {} {}: Stale (directory missing)",
                style("x").red(),
                branch_name
            );
            *issues += 1;
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

    Ok((worktrees, stale_count))
}

/// Check for uncommitted changes in worktrees.
fn check_uncommitted_changes(worktrees: &[WtInfo], warnings: &mut u32) {
    println!("{}", style("3. Checking for uncommitted changes...").bold());
    let mut dirty: Vec<String> = Vec::new();
    for wt in worktrees {
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
        *warnings += 1;
    }
    println!();
}

/// Check if worktrees are behind their base branch.
fn check_behind_base(
    worktrees: &[WtInfo],
    repo: &std::path::Path,
    warnings: &mut u32,
) -> Vec<(String, String, String)> {
    println!(
        "{}",
        style("4. Checking if worktrees are behind base branch...").bold()
    );
    let mut behind: Vec<(String, String, String)> = Vec::new();

    for wt in worktrees {
        if wt.status == "stale" {
            continue;
        }
        let key = format_config_key(CONFIG_KEY_BASE_BRANCH, &wt.branch);
        let base = match git::get_config(&key, Some(repo)) {
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
            style("Tip: Use 'gw sync --all' to update all worktrees").dim()
        );
        *warnings += 1;
    }
    println!();

    behind
}

/// Check for merge conflicts in worktrees.
fn check_merge_conflicts(worktrees: &[WtInfo], issues: &mut u32) -> Vec<(String, usize)> {
    println!("{}", style("5. Checking for merge conflicts...").bold());
    let mut conflicted: Vec<(String, usize)> = Vec::new();

    for wt in worktrees {
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
        *issues += 1;
    }
    println!();

    conflicted
}

/// Print health check summary.
fn print_summary(issues: u32, warnings: u32) {
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
}

/// Print remediation recommendations.
fn print_recommendations(
    stale_count: u32,
    behind: &[(String, String, String)],
    conflicted: &[(String, usize)],
) {
    let has_recommendations = stale_count > 0 || !behind.is_empty() || !conflicted.is_empty();
    if has_recommendations {
        println!("{}", style("Recommendations:").bold());
        if stale_count > 0 {
            println!(
                "  - Run {} to clean up stale worktrees",
                style("gw prune").cyan()
            );
        }
        if !behind.is_empty() {
            println!(
                "  - Run {} to update all worktrees",
                style("gw sync --all").cyan()
            );
        }
        if !conflicted.is_empty() {
            println!("  - Resolve conflicts in conflicted worktrees");
        }
        println!();
    }
}
