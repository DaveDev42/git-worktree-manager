/// Helper functions shared across operations modules.
///
/// Mirrors src/git_worktree_manager/operations/helpers.py (444 lines).
use std::path::{Path, PathBuf};

use crate::constants::{format_config_key, CONFIG_KEY_BASE_BRANCH, CONFIG_KEY_BASE_PATH};
use crate::error::{CwError, Result};
use crate::git;
use crate::messages;

// Thread-local global mode flag.
std::thread_local! {
    static GLOBAL_MODE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub fn set_global_mode(enabled: bool) {
    GLOBAL_MODE.with(|g| g.set(enabled));
}

pub fn is_global_mode() -> bool {
    GLOBAL_MODE.with(|g| g.get())
}

/// Parse 'repo:branch' notation.
pub fn parse_repo_branch_target(target: &str) -> (Option<&str>, &str) {
    if let Some((repo, branch)) = target.split_once(':') {
        if !repo.is_empty() && !branch.is_empty() {
            return (Some(repo), branch);
        }
    }
    (None, target)
}

/// Get the branch for a worktree path from parse_worktrees output.
pub fn get_branch_for_worktree(repo: &Path, worktree_path: &Path) -> Option<String> {
    let worktrees = git::parse_worktrees(repo).ok()?;
    let resolved = worktree_path
        .canonicalize()
        .unwrap_or_else(|_| worktree_path.to_path_buf());

    for (branch, path) in &worktrees {
        let p_resolved = path.canonicalize().unwrap_or_else(|_| path.clone());
        if p_resolved == resolved {
            if branch == "(detached)" {
                return None;
            }
            return Some(git::normalize_branch_name(branch).to_string());
        }
    }
    None
}

/// Resolve worktree target to (worktree_path, branch_name, worktree_repo).
///
/// Supports branch name lookup, worktree directory name lookup,
/// and disambiguation when both match.
pub fn resolve_worktree_target(
    target: Option<&str>,
    lookup_mode: Option<&str>,
) -> Result<(PathBuf, String, PathBuf)> {
    if target.is_none() && is_global_mode() {
        return Err(CwError::WorktreeNotFound(
            "Global mode requires an explicit target (branch or worktree name).".to_string(),
        ));
    }

    if target.is_none() {
        // Use current directory
        let cwd = std::env::current_dir()?;
        let branch = git::get_current_branch(Some(&cwd))?;
        let repo = git::get_repo_root(Some(&cwd))?;
        return Ok((cwd, branch, repo));
    }

    let target = target.unwrap();

    // Global mode: search all registered repositories
    if is_global_mode() {
        return resolve_global_target(target, lookup_mode);
    }

    let main_repo = git::get_main_repo_root(None)?;

    // Try branch lookup (skip if lookup_mode is "worktree")
    let branch_match = if lookup_mode != Some("worktree") {
        git::find_worktree_by_intended_branch(&main_repo, target)?
    } else {
        None
    };

    // Try worktree name lookup (skip if lookup_mode is "branch")
    let worktree_match = if lookup_mode != Some("branch") {
        git::find_worktree_by_name(&main_repo, target)?
    } else {
        None
    };

    match (branch_match, worktree_match) {
        (Some(bp), Some(wp)) => {
            let bp_resolved = bp.canonicalize().unwrap_or_else(|_| bp.clone());
            let wp_resolved = wp.canonicalize().unwrap_or_else(|_| wp.clone());
            if bp_resolved == wp_resolved {
                let repo = git::get_repo_root(Some(&bp))?;
                Ok((bp, target.to_string(), repo))
            } else {
                // Ambiguous — in non-interactive mode, prefer branch match
                if git::is_non_interactive() {
                    let repo = git::get_repo_root(Some(&bp))?;
                    Ok((bp, target.to_string(), repo))
                } else {
                    // Default to branch match
                    let repo = git::get_repo_root(Some(&bp))?;
                    Ok((bp, target.to_string(), repo))
                }
            }
        }
        (Some(bp), None) => {
            let repo = git::get_repo_root(Some(&bp))?;
            Ok((bp, target.to_string(), repo))
        }
        (None, Some(wp)) => {
            let branch =
                get_branch_for_worktree(&main_repo, &wp).unwrap_or_else(|| target.to_string());
            let repo = git::get_repo_root(Some(&wp))?;
            Ok((wp, branch, repo))
        }
        (None, None) => Err(CwError::WorktreeNotFound(messages::worktree_not_found(
            target,
        ))),
    }
}

/// Global mode target resolution.
fn resolve_global_target(
    target: &str,
    lookup_mode: Option<&str>,
) -> Result<(PathBuf, String, PathBuf)> {
    let repos = crate::registry::get_all_registered_repos();
    let (repo_filter, branch_target) = parse_repo_branch_target(target);

    for (name, repo_path) in &repos {
        if let Some(filter) = repo_filter {
            if name != filter {
                continue;
            }
        }
        if !repo_path.exists() {
            continue;
        }

        // Try branch lookup (skip if lookup_mode is "worktree")
        if lookup_mode != Some("worktree") {
            if let Ok(Some(path)) = git::find_worktree_by_intended_branch(repo_path, branch_target)
            {
                let repo = git::get_repo_root(Some(&path)).unwrap_or(repo_path.clone());
                return Ok((path, branch_target.to_string(), repo));
            }
        }

        // Try worktree name lookup (skip if lookup_mode is "branch")
        if lookup_mode != Some("branch") {
            if let Ok(Some(path)) = git::find_worktree_by_name(repo_path, branch_target) {
                let branch = get_branch_for_worktree(repo_path, &path)
                    .unwrap_or_else(|| branch_target.to_string());
                let repo = git::get_repo_root(Some(&path)).unwrap_or(repo_path.clone());
                return Ok((path, branch, repo));
            }
        }
    }

    Err(CwError::WorktreeNotFound(format!(
        "'{}' not found in any registered repository. Run 'gw scan' to register repos.",
        target
    )))
}

/// Get worktree metadata (base branch and base repository path).
///
/// If metadata is missing, tries to infer from common defaults.
pub fn get_worktree_metadata(branch: &str, repo: &Path) -> Result<(String, PathBuf)> {
    let base_key = format_config_key(CONFIG_KEY_BASE_BRANCH, branch);
    let path_key = format_config_key(CONFIG_KEY_BASE_PATH, branch);

    let base_branch = git::get_config(&base_key, Some(repo));
    let base_path_str = git::get_config(&path_key, Some(repo));

    if let (Some(bb), Some(bp)) = (base_branch, base_path_str) {
        return Ok((bb, PathBuf::from(bp)));
    }

    // Metadata missing — try to infer
    eprintln!(
        "Warning: Metadata missing for branch '{}'. Attempting to infer...",
        branch
    );

    // Infer base_path from first worktree entry
    let worktrees = git::parse_worktrees(repo)?;
    let inferred_base_path = worktrees.first().map(|(_, p)| p.clone()).ok_or_else(|| {
        CwError::Git(format!(
            "Cannot infer base repository path for branch '{}'. Use 'gw new' to create worktrees.",
            branch
        ))
    })?;

    // Infer base_branch from common defaults
    let mut inferred_base_branch: Option<String> = None;
    for candidate in &["main", "master", "develop"] {
        if git::branch_exists(candidate, Some(&inferred_base_path)) {
            inferred_base_branch = Some(candidate.to_string());
            break;
        }
    }

    if inferred_base_branch.is_none() {
        if let Some((first_branch, _)) = worktrees.first() {
            if first_branch != "(detached)" {
                inferred_base_branch = Some(git::normalize_branch_name(first_branch).to_string());
            }
        }
    }

    let base = inferred_base_branch.ok_or_else(|| {
        CwError::Git(format!(
            "Cannot infer base branch for '{}'. Use 'gw new' to create worktrees.",
            branch
        ))
    })?;

    eprintln!("  Inferred base branch: {}", base);
    eprintln!("  Inferred base path: {}", inferred_base_path.display());

    Ok((base, inferred_base_path))
}
