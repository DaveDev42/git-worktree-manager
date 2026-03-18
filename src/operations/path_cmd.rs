/// Internal _path command for shell function integration.
///
/// Mirrors the _path command in cli.py — used by cw-cd shell function.
use crate::error::{CwError, Result};
use crate::git;
use crate::registry;

/// Resolve branch to path (outputs to stdout for shell consumption).
pub fn worktree_path(
    branch: Option<&str>,
    global_mode: bool,
    list_branches: bool,
    interactive: bool,
) -> Result<()> {
    if interactive {
        return interactive_path_selection(global_mode);
    }

    if list_branches {
        return list_branch_names(global_mode);
    }

    let branch = branch.ok_or_else(|| {
        CwError::Git(
            "branch argument is required (unless --list-branches or --interactive is used)"
                .to_string(),
        )
    })?;

    if global_mode {
        return resolve_global_path(branch);
    }

    // Local mode
    let repo = git::get_repo_root(None)?;
    let normalized = git::normalize_branch_name(branch);
    let path = git::find_worktree_by_branch(&repo, branch)?
        .or(git::find_worktree_by_branch(
            &repo,
            &format!("refs/heads/{}", normalized),
        )?)
        .ok_or_else(|| CwError::Git(format!("No worktree found for branch '{}'", branch)))?;

    println!("{}", path.display());
    Ok(())
}

fn list_branch_names(global_mode: bool) -> Result<()> {
    if global_mode {
        let repos = registry::get_all_registered_repos();
        for (name, repo_path) in &repos {
            if !repo_path.exists() {
                continue;
            }
            if let Ok(worktrees) = git::get_feature_worktrees(Some(repo_path)) {
                for (branch, _) in &worktrees {
                    println!("{}:{}", name, branch);
                }
            }
        }
    } else if let Ok(repo) = git::get_repo_root(None) {
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

fn resolve_global_path(branch: &str) -> Result<()> {
    let repos = registry::get_all_registered_repos();

    // Parse repo:branch notation
    let (repo_filter, branch_target) = if let Some((r, b)) = branch.split_once(':') {
        (Some(r), b)
    } else {
        (None, branch)
    };

    let mut matches: Vec<(std::path::PathBuf, String, String)> = Vec::new();

    for (name, repo_path) in &repos {
        if let Some(filter) = repo_filter {
            if name != filter {
                continue;
            }
        }
        if !repo_path.exists() {
            continue;
        }

        if let Ok(Some(path)) = git::find_worktree_by_branch(repo_path, branch_target) {
            matches.push((path, branch_target.to_string(), name.clone()));
        } else if let Ok(Some(path)) =
            git::find_worktree_by_branch(repo_path, &format!("refs/heads/{}", branch_target))
        {
            matches.push((path, branch_target.to_string(), name.clone()));
        }
    }

    if matches.is_empty() {
        return Err(CwError::Git(format!(
            "No worktree found for '{}' in any registered repository",
            branch
        )));
    }

    if matches.len() == 1 {
        println!("{}", matches[0].0.display());
        return Ok(());
    }

    // Multiple matches
    eprintln!("Multiple worktrees found for '{}':", branch);
    for (path, branch_name, repo_name) in &matches {
        eprintln!("  {}:{}  ({})", repo_name, branch_name, path.display());
    }
    eprintln!("Use 'repo:branch' notation to disambiguate.");
    Err(CwError::Git(format!(
        "Multiple worktrees found for '{}'",
        branch
    )))
}

fn interactive_path_selection(global_mode: bool) -> Result<()> {
    let mut entries: Vec<(String, String)> = Vec::new(); // (label, path)

    if global_mode {
        let repos = registry::get_all_registered_repos();
        for (name, repo_path) in &repos {
            if !repo_path.exists() {
                continue;
            }
            if let Ok(worktrees) = git::parse_worktrees(repo_path) {
                let repo_resolved = repo_path
                    .canonicalize()
                    .unwrap_or_else(|_| repo_path.clone());
                for (branch, path) in &worktrees {
                    let normalized = git::normalize_branch_name(branch);
                    let path_resolved = path.canonicalize().unwrap_or_else(|_| path.clone());
                    if path_resolved == repo_resolved {
                        entries.insert(
                            0,
                            (
                                format!("{} (root)", name),
                                path.to_string_lossy().to_string(),
                            ),
                        );
                    } else if normalized != "(detached)" {
                        entries.push((
                            format!("{}:{}", name, normalized),
                            path.to_string_lossy().to_string(),
                        ));
                    }
                }
            }
        }
    } else {
        let repo = git::get_main_repo_root(None)?;
        let worktrees = git::parse_worktrees(&repo)?;
        let repo_resolved = repo.canonicalize().unwrap_or_else(|_| repo.clone());

        for (branch, path) in &worktrees {
            let normalized = git::normalize_branch_name(branch);
            let path_resolved = path.canonicalize().unwrap_or_else(|_| path.clone());
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
    }

    if entries.is_empty() {
        eprintln!("No worktrees found.");
        std::process::exit(1);
    }

    if entries.len() == 1 {
        println!("{}", entries[0].1);
        return Ok(());
    }

    // Simple numbered selection (stderr for UI, stdout for path)
    if !atty_stderr() {
        return Err(CwError::Git(
            "Interactive mode requires a terminal (TTY)".to_string(),
        ));
    }

    eprintln!("Select worktree:");
    for (i, (label, _)) in entries.iter().enumerate() {
        eprintln!("  [{}] {}", i + 1, label);
    }
    eprint!("Choice [1-{}]: ", entries.len());

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let choice: usize = input.trim().parse().unwrap_or(0);

    if choice >= 1 && choice <= entries.len() {
        println!("{}", entries[choice - 1].1);
        Ok(())
    } else {
        std::process::exit(1);
    }
}

fn atty_stderr() -> bool {
    std::io::IsTerminal::is_terminal(&std::io::stderr())
}
