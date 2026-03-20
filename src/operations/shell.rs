/// Shell command — open interactive shell or execute command in a worktree.
///
/// Mirrors shell_worktree from ai_tools.py.
use std::path::PathBuf;

use console::style;

use crate::error::{CwError, Result};
use crate::git;
use crate::messages;

/// Open interactive shell or execute command in a worktree.
pub fn shell_worktree(worktree: Option<&str>, command: Option<Vec<String>>) -> Result<()> {
    let target_path: PathBuf;

    if let Some(wt) = worktree {
        let repo = git::get_repo_root(None)?;
        let path = git::find_worktree_by_branch(&repo, wt)?
            .or(git::find_worktree_by_branch(
                &repo,
                &format!("refs/heads/{}", git::normalize_branch_name(wt)),
            )?)
            .ok_or_else(|| CwError::WorktreeNotFound(messages::worktree_not_found(wt)))?;
        target_path = path;
    } else {
        target_path = std::env::current_dir()?;
        // Verify we're in a git repo
        let _ = git::get_current_branch(Some(&target_path))?;
    }

    if !target_path.exists() {
        return Err(CwError::WorktreeNotFound(messages::worktree_dir_not_found(
            &target_path.display().to_string(),
        )));
    }

    if let Some(cmd) = command {
        if cmd.is_empty() {
            return open_shell(&target_path, worktree);
        }
        // Execute command
        println!(
            "{} {}\n",
            style(format!("Executing in {}:", target_path.display())).cyan(),
            cmd.join(" ")
        );

        let status = std::process::Command::new(&cmd[0])
            .args(&cmd[1..])
            .current_dir(&target_path)
            .status()?;

        std::process::exit(status.code().unwrap_or(1));
    } else {
        open_shell(&target_path, worktree)?;
    }

    Ok(())
}

fn open_shell(path: &std::path::Path, branch: Option<&str>) -> Result<()> {
    let branch_name = branch
        .map(|b| b.to_string())
        .or_else(|| git::get_current_branch(Some(path)).ok())
        .unwrap_or_else(|| "unknown".to_string());

    println!(
        "{} {}\n{}\n{}\n",
        style("Opening shell in worktree:").cyan().bold(),
        branch_name,
        style(format!("Path: {}", path.display())).dim(),
        style("Type 'exit' to return").dim(),
    );

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());

    let _ = std::process::Command::new(&shell)
        .current_dir(path)
        .status();

    Ok(())
}
