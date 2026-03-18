/// Stash operations for git-worktree-manager.
///
/// Mirrors src/git_worktree_manager/operations/stash_ops.py.
use std::collections::HashMap;

use console::style;

use crate::error::{CwError, Result};
use crate::git;

/// Save changes in current worktree to stash.
pub fn stash_save(message: Option<&str>) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let branch = git::get_current_branch(Some(&cwd))?;

    let stash_msg = match message {
        Some(m) => format!("[{}] {}", branch, m),
        None => format!("[{}] WIP", branch),
    };

    // Check for changes
    let status = git::git_command(&["status", "--porcelain"], Some(&cwd), false, true)?;
    if status.returncode == 0 && status.stdout.trim().is_empty() {
        println!("{} No changes to stash\n", style("!").yellow());
        return Ok(());
    }

    println!(
        "{}",
        style(format!("Stashing changes in {}...", branch)).yellow()
    );
    git::git_command(
        &["stash", "push", "--include-untracked", "-m", &stash_msg],
        Some(&cwd),
        true,
        false,
    )?;
    println!(
        "{} Stashed changes: {}\n",
        style("*").green().bold(),
        stash_msg
    );

    Ok(())
}

/// List all stashes organized by worktree/branch.
pub fn stash_list() -> Result<()> {
    let repo = git::get_repo_root(None)?;
    let result = git::git_command(&["stash", "list"], Some(&repo), false, true)?;

    if result.stdout.trim().is_empty() {
        println!("{}\n", style("No stashes found").yellow());
        return Ok(());
    }

    println!("\n{}\n", style("Stashes by worktree:").cyan().bold());

    let mut by_branch: HashMap<String, Vec<(String, String)>> = HashMap::new();

    for line in result.stdout.trim().lines() {
        let parts: Vec<&str> = line.splitn(3, ':').collect();
        if parts.len() < 3 {
            continue;
        }

        let stash_ref = parts[0].trim();
        let stash_info = parts[1].trim();
        let stash_msg = parts[2].trim();

        // Extract branch name from our format [branch-name] or from git format
        let branch_name = if stash_msg.starts_with('[') && stash_msg.contains(']') {
            let end = stash_msg.find(']').unwrap();
            stash_msg[1..end].to_string()
        } else if stash_info.contains("On ") {
            stash_info
                .split("On ")
                .nth(1)
                .unwrap_or("unknown")
                .trim()
                .to_string()
        } else if stash_info.contains("WIP on ") {
            stash_info
                .split("WIP on ")
                .nth(1)
                .unwrap_or("unknown")
                .trim()
                .to_string()
        } else {
            "unknown".to_string()
        };

        by_branch
            .entry(branch_name)
            .or_default()
            .push((stash_ref.to_string(), stash_msg.to_string()));
    }

    let mut branches: Vec<_> = by_branch.keys().cloned().collect();
    branches.sort();

    for branch in branches {
        println!("{}:", style(&branch).green().bold());
        for (stash_ref, msg) in &by_branch[&branch] {
            println!("  {}: {}", stash_ref, msg);
        }
        println!();
    }

    Ok(())
}

/// Apply a stash to a different worktree.
pub fn stash_apply(target_branch: &str, stash_ref: &str) -> Result<()> {
    let repo = git::get_repo_root(None)?;

    let wt_path = git::find_worktree_by_branch(&repo, target_branch)?
        .or(git::find_worktree_by_branch(
            &repo,
            &format!("refs/heads/{}", target_branch),
        )?)
        .ok_or_else(|| {
            CwError::WorktreeNotFound(format!(
                "No worktree found for branch '{}'. Use 'cw list' to see available worktrees.",
                target_branch
            ))
        })?;

    // Verify stash exists
    let verify = git::git_command(&["stash", "list"], Some(&repo), false, true)?;
    if !verify.stdout.contains(stash_ref) {
        return Err(CwError::Git(format!(
            "Stash '{}' not found. Use 'cw stash list' to see available stashes.",
            stash_ref
        )));
    }

    println!(
        "\n{}",
        style(format!("Applying {} to {}...", stash_ref, target_branch)).yellow()
    );

    match git::git_command(&["stash", "apply", stash_ref], Some(&wt_path), false, true) {
        Ok(r) if r.returncode == 0 => {
            println!(
                "{} Stash applied to {}\n",
                style("*").green().bold(),
                target_branch
            );
            println!(
                "{}\n",
                style(format!("Worktree path: {}", wt_path.display())).dim()
            );
        }
        _ => {
            println!("{} Failed to apply stash\n", style("x").red().bold());
            println!(
                "{}\n",
                style("Tip: There may be conflicts. Check the worktree and resolve manually.")
                    .yellow()
            );
        }
    }

    Ok(())
}
