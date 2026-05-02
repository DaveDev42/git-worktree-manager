/// Internal _path command for shell function integration.
///
/// Mirrors the _path command in cli.py — used by cw-cd shell function.
use crate::error::{CwError, Result};
use crate::git;
use crate::messages;

/// Resolve branch to path (outputs to stdout for shell consumption).
pub fn worktree_path(branch: Option<&str>, list_branches: bool, interactive: bool) -> Result<()> {
    if interactive {
        return interactive_path_selection();
    }

    if list_branches {
        return list_branch_names();
    }

    let branch = branch.ok_or_else(|| {
        CwError::Git(
            "branch argument is required (unless --list-branches or --interactive is used)"
                .to_string(),
        )
    })?;

    // Local mode
    let repo = git::get_repo_root(None)?;
    let normalized = git::normalize_branch_name(branch);
    let path = git::find_worktree_by_branch(&repo, branch)?
        .or(git::find_worktree_by_branch(
            &repo,
            &format!("refs/heads/{}", normalized),
        )?)
        .ok_or_else(|| CwError::Git(messages::worktree_not_found(branch)))?;

    println!("{}", path.display());
    Ok(())
}

fn list_branch_names() -> Result<()> {
    if let Ok(repo) = git::get_repo_root(None) {
        if let Ok(worktrees) = git::parse_worktrees(&repo) {
            for (branch, _) in &worktrees {
                let normalized = git::normalize_branch_name(branch);
                if normalized != "(detached)" {
                    println!("{}", normalized);
                }
            }
        }
    }
    Ok(())
}

fn interactive_path_selection() -> Result<()> {
    let mut entries: Vec<(String, String)> = Vec::new(); // (label, path)

    let repo = git::get_main_repo_root(None)?;
    let worktrees = git::parse_worktrees(&repo)?;
    let repo_resolved = git::canonicalize_or(&repo);

    for (branch, path) in &worktrees {
        let normalized = git::normalize_branch_name(branch);
        let path_resolved = git::canonicalize_or(path);
        if path_resolved == repo_resolved {
            let label = if normalized.is_empty() || normalized == "(detached)" {
                "main (root)".to_string()
            } else {
                format!("{} (root)", normalized)
            };
            entries.insert(0, (label, path.to_string_lossy().to_string()));
        } else if normalized != "(detached)" {
            entries.push((normalized.to_string(), path.to_string_lossy().to_string()));
        }
    }

    if entries.is_empty() {
        eprintln!("No worktrees found.");
        std::process::exit(1);
    }

    if entries.len() == 1 {
        println!("{}", entries[0].1);
        return Ok(());
    }

    // Try arrow-key TUI selector, fall back to numbered selection
    match crate::tui::arrow_select(&entries, "Select worktree:", 0) {
        Some(selected_path) => {
            println!("{}", selected_path);
            Ok(())
        }
        None => {
            std::process::exit(1);
        }
    }
}
