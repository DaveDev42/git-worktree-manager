//! Batch deletion orchestration for `gw delete`.
//!
//! Multi-target deletion pipeline: resolve-all → plan (busy) → summary →
//! confirm → execute → exit code. Reuses `worktree::delete_one` for per-target
//! execution.

use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::git;
use crate::operations::busy::{self, BusyInfo};

/// Resolved worktree target (path + optional branch).
#[derive(Debug, Clone)]
pub struct Resolved {
    pub input: String,
    pub path: PathBuf,
    pub branch: Option<String>,
}

/// A single entry in the batch execution plan.
#[derive(Debug)]
pub enum PlanEntry {
    Ready(Resolved),
    Busy {
        resolved: Resolved,
        info: Vec<BusyInfo>,
    },
    Unresolved {
        input: String,
        reason: String,
    },
}

/// Resolve a list of user inputs against the main repository.
///
/// Inputs may be branch names, worktree directory names, or filesystem paths.
/// Anything that does not resolve becomes a `PlanEntry::Unresolved`.
pub fn resolve_all(inputs: &[String], lookup_mode: Option<&str>) -> Result<Vec<PlanEntry>> {
    let main_repo = git::get_main_repo_root(None)?;
    let mut out = Vec::with_capacity(inputs.len());
    for input in inputs {
        match resolve_one(input, &main_repo, lookup_mode) {
            Some(resolved) => out.push(PlanEntry::Ready(resolved)),
            None => out.push(PlanEntry::Unresolved {
                input: input.clone(),
                reason: "not found".into(),
            }),
        }
    }
    Ok(out)
}

fn resolve_one(input: &str, main_repo: &Path, lookup_mode: Option<&str>) -> Option<Resolved> {
    // 1) filesystem path
    let p = PathBuf::from(input);
    if p.exists() {
        let resolved = p.canonicalize().unwrap_or(p);
        let branch = crate::operations::helpers::get_branch_for_worktree(main_repo, &resolved);
        return Some(Resolved {
            input: input.to_string(),
            path: resolved,
            branch,
        });
    }

    // 2) branch lookup
    if lookup_mode != Some("worktree") {
        if let Ok(Some(path)) = git::find_worktree_by_intended_branch(main_repo, input) {
            return Some(Resolved {
                input: input.to_string(),
                path,
                branch: Some(input.to_string()),
            });
        }
    }

    // 3) worktree name lookup
    if lookup_mode != Some("branch") {
        if let Ok(Some(path)) = git::find_worktree_by_name(main_repo, input) {
            let branch = crate::operations::helpers::get_branch_for_worktree(main_repo, &path);
            return Some(Resolved {
                input: input.to_string(),
                path,
                branch,
            });
        }
    }

    None
}

/// Annotate resolved entries with busy status. Unresolved entries pass through.
pub fn plan_busy(entries: Vec<PlanEntry>, allow_busy: bool) -> Vec<PlanEntry> {
    if allow_busy {
        return entries;
    }
    entries
        .into_iter()
        .map(|entry| match entry {
            PlanEntry::Ready(r) => {
                let info = busy::detect_busy(&r.path);
                if info.is_empty() {
                    PlanEntry::Ready(r)
                } else {
                    PlanEntry::Busy { resolved: r, info }
                }
            }
            other => other,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_busy_passthrough_when_allowed() {
        let entries = vec![PlanEntry::Unresolved {
            input: "x".into(),
            reason: "not found".into(),
        }];
        let out = plan_busy(entries, true);
        assert_eq!(out.len(), 1);
        assert!(matches!(out[0], PlanEntry::Unresolved { .. }));
    }
}
