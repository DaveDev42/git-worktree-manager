//! cwd-based scope discovery for gw.
//!
//! Determines which worktrees a gw command should act on based on the
//! invocation directory. Two cases:
//!
//! 1. cwd inside a git worktree → return that worktree's family (main repo +
//!    all linked worktrees).
//! 2. cwd outside any repo → walk down (depth ≤ 4, skipping dotdirs and
//!    common build dirs) and gather every distinct repo family found.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::error::{CwError, Result};
use crate::git;

#[derive(Debug, Clone)]
pub struct ScopedWorktree {
    /// Basename of the worktree directory.
    pub name: String,
    /// Short branch name. `None` for detached HEAD.
    pub branch: Option<String>,
    /// Absolute path to the worktree directory.
    pub path: PathBuf,
    /// Main repo root this worktree belongs to.
    pub repo_root: PathBuf,
    /// True if this worktree is itself the main repo of its family.
    pub is_main: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Scope {
    worktrees: Vec<ScopedWorktree>,
}

impl Scope {
    pub fn worktrees(&self) -> &[ScopedWorktree] {
        &self.worktrees
    }

    pub fn is_empty(&self) -> bool {
        self.worktrees.is_empty()
    }
}

const MAX_WALK_DEPTH: usize = 4;

pub fn discover_scope(cwd: &Path) -> Result<Scope> {
    if let Ok(repo_root) = git::get_main_repo_root(Some(cwd)) {
        let worktrees = collect_family(&repo_root)?;
        return Ok(Scope { worktrees });
    }

    let mut all = Vec::new();
    let mut seen_roots: HashSet<PathBuf> = HashSet::new();
    walk_for_repos(cwd, 0, &mut all, &mut seen_roots)?;

    if all.is_empty() {
        return Err(CwError::Other(
            "no worktrees in scope (cwd is not in a git repo and no repos found below)".into(),
        ));
    }
    Ok(Scope { worktrees: all })
}

fn collect_family(repo_root: &Path) -> Result<Vec<ScopedWorktree>> {
    let raw = git::parse_worktrees(repo_root)?;
    let main_canon = git::canonicalize_or(repo_root);
    let mut out = Vec::with_capacity(raw.len());
    for (branch_raw, path) in raw {
        let normalized = git::normalize_branch_name(&branch_raw);
        let branch = if normalized == "(detached)" {
            None
        } else {
            Some(normalized.to_string())
        };
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let is_main = git::canonicalize_or(&path) == main_canon;
        out.push(ScopedWorktree {
            name,
            branch,
            path,
            repo_root: repo_root.to_path_buf(),
            is_main,
        });
    }
    Ok(out)
}

fn walk_for_repos(
    dir: &Path,
    depth: usize,
    out: &mut Vec<ScopedWorktree>,
    seen: &mut HashSet<PathBuf>,
) -> Result<()> {
    if depth > MAX_WALK_DEPTH {
        return Ok(());
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
            if name.starts_with('.') && name != ".git" {
                continue;
            }
            if name == "node_modules" || name == "target" {
                continue;
            }
        }
        if let Ok(repo) = git::get_main_repo_root(Some(&path)) {
            if seen.insert(repo.clone()) {
                out.extend(collect_family(&repo)?);
            }
            continue;
        }
        walk_for_repos(&path, depth + 1, out, seen)?;
    }
    Ok(())
}
