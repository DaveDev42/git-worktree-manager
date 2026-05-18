use std::process::Command;

use console::style;

use crate::constants::{
    format_config_key, version_meets_minimum, CONFIG_KEY_BASE_BRANCH, MIN_GIT_VERSION,
    MIN_GIT_VERSION_MAJOR, MIN_GIT_VERSION_MINOR,
};
use crate::error::Result;
use crate::git;

use super::display::get_worktree_status;
use super::pr_cache::PrCache;
use super::setup_claude;

/// Worktree info collected during health check.
struct WtInfo {
    branch: String,
    path: std::path::PathBuf,
    status: String,
}

/// Perform health check on all worktrees.
pub fn doctor(session_start: bool, quiet: bool) -> Result<()> {
    if session_start {
        return doctor_session_start(quiet);
    }
    let repo = git::get_repo_root(None)?;
    println!(
        "\n{}\n",
        style("git-worktree-manager Health Check").cyan().bold()
    );

    let mut issues = 0u32;
    let mut warnings = 0u32;

    check_git_version(&mut issues);
    let (worktrees, stale_count) = check_worktree_accessibility(&repo, &mut issues)?;
    check_uncommitted_changes(&worktrees, &mut warnings);
    check_busy_worktrees(&worktrees);
    check_claude_integration();

    print_summary(issues, warnings);
    print_recommendations(stale_count);

    Ok(())
}

/// Hook-friendly single-line health summary. Always returns Ok(()) so a
/// SessionStart hook never blocks the Claude Code session.
fn doctor_session_start(quiet: bool) -> Result<()> {
    let cwd = std::env::current_dir().ok();
    let cwd_ok = cwd.as_ref().map(|p| p.exists()).unwrap_or(false);
    let cwd_str = cwd
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "?".into());

    // Branch + base are best-effort: failures here must not abort the
    // line. Each failed lookup contributes "?" to the output.
    let repo_root = git::get_repo_root(None).ok();
    let branch = repo_root
        .as_deref()
        .and_then(|root| git::get_current_branch(Some(root)).ok())
        .unwrap_or_else(|| "?".into());
    let base = if branch != "?" {
        repo_root
            .as_deref()
            .and_then(|root| {
                let key = format_config_key(CONFIG_KEY_BASE_BRANCH, &branch);
                git::get_config(&key, Some(root))
            })
            .unwrap_or_else(|| "?".into())
    } else {
        "?".into()
    };
    let prefix = if quiet { "gw:" } else { "gw doctor:" };
    println!(
        "{} cwd={} ok={} branch={} base={}",
        prefix, cwd_str, cwd_ok, branch, base,
    );
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

    // Use a default (empty) cache — stale detection depends only on path
    // existence, not on PR state, so there is no need for a gh call here.
    let pr_cache = PrCache::default();

    for (branch_name, path) in &feature_worktrees {
        let status = get_worktree_status(path, repo, Some(branch_name.as_str()), &pr_cache);
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

/// Check for worktrees with active sessions in other shells.
/// This is informational — it tells the user where parallel work is
/// happening, not whether something is wrong.
fn check_busy_worktrees(worktrees: &[WtInfo]) {
    println!("{}", style("4. Checking busy worktrees...").bold());
    let busy: Vec<&WtInfo> = worktrees
        .iter()
        .filter(|w| !crate::operations::busy::detect_busy_lockfile_only(&w.path).is_empty())
        .collect();
    if busy.is_empty() {
        println!("   {} No busy worktrees", style("*").green());
    } else {
        println!(
            "   {} {} busy worktree(s) (active in another session):",
            style("i").cyan(),
            busy.len()
        );
        for wt in &busy {
            println!("      - {}", wt.branch);
        }
    }
    println!();
}

/// Check Claude Code installation and skill integration.
fn check_claude_integration() {
    println!("{}", style("5. Checking Claude Code integration...").bold());

    let has_claude = Command::new("which")
        .arg("claude")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    let msgs = setup_claude_doctor_messages();

    if !has_claude {
        println!(
            "   {} Claude Code not detected (optional)",
            style("-").dim()
        );
    } else if setup_claude::is_installed() {
        println!("   {} {}", style("*").green(), msgs.installed);
    } else {
        println!("   {} {}", style("!").yellow(), msgs.missing_alert);
        println!("   {}", style(format!("Tip: {}", msgs.missing_tip)).dim());
    }
    println!();
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
fn print_recommendations(stale_count: u32) {
    if stale_count == 0 {
        return;
    }
    println!("{}", style("Recommendations:").bold());
    println!(
        "  - Run {} to clean up stale worktrees",
        style("gw rm").cyan()
    );
    println!();
}

/// Source-of-truth strings for the Claude Code section of `gw doctor`.
pub struct SetupClaudeDoctorMessages {
    pub installed: &'static str,
    pub legacy_alert: &'static str,
    pub legacy_tip: &'static str,
    pub missing_alert: &'static str,
    pub missing_tip: &'static str,
}

/// Returns the strings printed by [`check_claude_integration`].
pub fn setup_claude_doctor_messages() -> SetupClaudeDoctorMessages {
    SetupClaudeDoctorMessages {
        installed: "gw skills installed",
        legacy_alert: "Legacy gw install detected (pre-marketplace layout)",
        legacy_tip: "Re-run 'gw setup-claude' to refresh skill files and hooks",
        missing_alert: "Claude Code detected but gw skills not installed",
        missing_tip: "Run 'gw setup-claude' to install gw skills and hooks into this repo",
    }
}
