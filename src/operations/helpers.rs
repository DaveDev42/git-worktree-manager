/// Helper functions shared across operations modules.
///
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::constants::{format_config_key, CONFIG_KEY_BASE_BRANCH, CONFIG_KEY_BASE_PATH};
use crate::error::{CwError, Result};
use crate::git;
use crate::messages;

/// Resolved worktree target with named fields for clarity.
pub struct ResolvedTarget {
    pub path: PathBuf,
    pub branch: String,
    pub repo: PathBuf,
}

/// Strictly-resolved worktree target: name, branch, and path.
///
/// Produced by [`resolve_target_strict`].
#[derive(Debug)]
pub struct StrictTarget {
    /// Basename of the worktree directory (e.g. `"repo-feat-x"`).
    pub name: String,
    /// Short branch name (e.g. `"feat-x"`), with `refs/heads/` prefix stripped.
    pub branch: String,
    /// Absolute path to the worktree directory.
    pub path: PathBuf,
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
    let resolved = git::canonicalize_or(worktree_path);

    for (branch, path) in &worktrees {
        let p_resolved = git::canonicalize_or(path);
        if p_resolved == resolved {
            if branch == "(detached)" {
                return None;
            }
            return Some(git::normalize_branch_name(branch).to_string());
        }
    }
    None
}

/// Resolve worktree target to a [`ResolvedTarget`] with path, branch, and repo.
///
/// Supports branch name lookup, worktree directory name lookup,
/// and disambiguation when both match.
pub fn resolve_worktree_target(
    target: Option<&str>,
    lookup_mode: Option<&str>,
) -> Result<ResolvedTarget> {
    if target.is_none() {
        // Use current directory
        let cwd = std::env::current_dir()?;
        let branch = git::get_current_branch(Some(&cwd))?;
        let repo = git::get_repo_root(Some(&cwd))?;
        return Ok(ResolvedTarget {
            path: cwd,
            branch,
            repo,
        });
    }

    let target = target.unwrap();

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
            let bp_resolved = git::canonicalize_or(&bp);
            let wp_resolved = git::canonicalize_or(&wp);
            if bp_resolved == wp_resolved {
                let repo = git::get_repo_root(Some(&bp))?;
                Ok(ResolvedTarget {
                    path: bp,
                    branch: target.to_string(),
                    repo,
                })
            } else {
                // Ambiguous — prefer branch match
                let repo = git::get_repo_root(Some(&bp))?;
                Ok(ResolvedTarget {
                    path: bp,
                    branch: target.to_string(),
                    repo,
                })
            }
        }
        (Some(bp), None) => {
            let repo = git::get_repo_root(Some(&bp))?;
            Ok(ResolvedTarget {
                path: bp,
                branch: target.to_string(),
                repo,
            })
        }
        (None, Some(wp)) => {
            let branch =
                get_branch_for_worktree(&main_repo, &wp).unwrap_or_else(|| target.to_string());
            let repo = git::get_repo_root(Some(&wp))?;
            Ok(ResolvedTarget {
                path: wp,
                branch,
                repo,
            })
        }
        (None, None) => Err(CwError::WorktreeNotFound(messages::worktree_not_found(
            target,
        ))),
    }
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

/// Strict target resolution: exact worktree name → exact branch name → exact path.
///
/// Unlike [`resolve_worktree_target`], this function performs no fuzzy matching
/// and no metadata look-ups. It calls [`git::parse_worktrees`] once and applies
/// three ordered exact-match rules:
///
/// 1. **Worktree name** — basename of the worktree directory equals `target`.
/// 2. **Branch name** — short branch name (after stripping `refs/heads/`) equals `target`.
/// 3. **Absolute path** — the worktree path equals `target`.
///
/// Returns [`CwError::WorktreeNotFound`] when no worktree matches.
pub fn resolve_target_strict(repo_root: &Path, target: &str) -> Result<StrictTarget> {
    let worktrees = git::parse_worktrees(repo_root)?;

    // Pre-compute the main worktree path so we can skip it when doing name
    // lookup (the main repo's basename is rarely meaningful as a worktree name).
    let main_path = worktrees
        .first()
        .map(|(_, p)| git::canonicalize_or(p))
        .unwrap_or_default();

    // Helper: build a StrictTarget from a raw parse_worktrees entry.
    let make = |(branch_raw, path): &(String, PathBuf)| -> StrictTarget {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let branch = git::normalize_branch_name(branch_raw).to_string();
        StrictTarget {
            name,
            branch,
            path: path.clone(),
        }
    };

    // 1) Exact worktree name (basename of path).
    for entry in &worktrees {
        let (_, path) = entry;
        // Skip main repo entry
        if git::canonicalize_or(path) == main_path {
            continue;
        }
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default();
        if name == target {
            return Ok(make(entry));
        }
    }

    // 2) Exact branch name (refs/heads/ prefix stripped).
    for entry in &worktrees {
        let (branch_raw, path) = entry;
        if git::canonicalize_or(path) == main_path {
            continue;
        }
        if git::normalize_branch_name(branch_raw) == target {
            return Ok(make(entry));
        }
    }

    // 3) Exact absolute path (canonicalize both sides to handle symlinks).
    let abs = PathBuf::from(target);
    let abs_resolved = git::canonicalize_or(&abs);
    for entry in &worktrees {
        let (_, path) = entry;
        let path_resolved = git::canonicalize_or(path);
        if path_resolved == abs_resolved {
            return Ok(make(entry));
        }
    }

    Err(CwError::WorktreeNotFound(messages::target_not_found(
        target,
    )))
}

/// Build a hook context HashMap with standard fields.
pub fn build_hook_context(
    branch: &str,
    base_branch: &str,
    worktree_path: &Path,
    repo_path: &Path,
    event: &str,
    operation: &str,
) -> HashMap<String, String> {
    HashMap::from([
        ("branch".into(), branch.to_string()),
        ("base_branch".into(), base_branch.to_string()),
        (
            "worktree_path".into(),
            worktree_path.to_string_lossy().to_string(),
        ),
        ("repo_path".into(), repo_path.to_string_lossy().to_string()),
        ("event".into(), event.to_string()),
        ("operation".into(), operation.to_string()),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_build_hook_context_all_fields() {
        let ctx = build_hook_context(
            "feat/login",
            "main",
            Path::new("/tmp/worktree"),
            Path::new("/tmp/repo"),
            "worktree.pre_create",
            "new",
        );

        assert_eq!(ctx.len(), 6);
        assert_eq!(ctx["branch"], "feat/login");
        assert_eq!(ctx["base_branch"], "main");
        assert_eq!(ctx["worktree_path"], "/tmp/worktree");
        assert_eq!(ctx["repo_path"], "/tmp/repo");
        assert_eq!(ctx["event"], "worktree.pre_create");
        assert_eq!(ctx["operation"], "new");
    }

    #[test]
    fn test_parse_repo_branch_target() {
        assert_eq!(
            parse_repo_branch_target("myrepo:feat/x"),
            (Some("myrepo"), "feat/x")
        );
        assert_eq!(parse_repo_branch_target("feat/x"), (None, "feat/x"));
        assert_eq!(parse_repo_branch_target(":feat/x"), (None, ":feat/x"));
        assert_eq!(parse_repo_branch_target("myrepo:"), (None, "myrepo:"));
    }
}
