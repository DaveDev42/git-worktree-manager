//! Batch deletion orchestration for `gw delete`.
//!
//! Multi-target deletion pipeline: resolve-all → plan (busy) → summary →
//! confirm → execute → exit code. Reuses `worktree::delete_one` for per-target
//! execution.

use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};

use console::style;

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

/// Counters for summary output.
struct PlanCounts {
    ready: usize,
    busy: usize,
    unresolved: usize,
}

fn count(entries: &[PlanEntry]) -> PlanCounts {
    let mut c = PlanCounts {
        ready: 0,
        busy: 0,
        unresolved: 0,
    };
    for e in entries {
        match e {
            PlanEntry::Ready(_) => c.ready += 1,
            PlanEntry::Busy { .. } => c.busy += 1,
            PlanEntry::Unresolved { .. } => c.unresolved += 1,
        }
    }
    c
}

/// Print the batch summary. Goes to stdout to match the convention used by
/// `gw clean` (summary/progress → stdout, errors/prompts → stderr).
pub fn print_summary(entries: &[PlanEntry], dry_run: bool) {
    let counts = count(entries);
    let header = if dry_run {
        format!("Would delete {} worktree(s):", counts.ready)
    } else {
        let busy_note = if counts.busy > 0 {
            format!(" ({} busy, will skip without --force)", counts.busy)
        } else {
            String::new()
        };
        format!("Deleting {} worktree(s){}:", counts.ready, busy_note)
    };
    println!("\n{}", style(header).yellow().bold());
    for e in entries {
        match e {
            PlanEntry::Ready(r) => {
                let label = r.branch.as_deref().unwrap_or(&r.input);
                println!("  {:<30} {}", label, r.path.display());
            }
            PlanEntry::Busy { resolved, info } => {
                let label = resolved.branch.as_deref().unwrap_or(&resolved.input);
                let detail = info
                    .first()
                    .map(|b| format!("PID {} {}", b.pid, b.cmd))
                    .unwrap_or_default();
                println!("  {:<30} (busy: {})  [skip]", label, detail);
            }
            PlanEntry::Unresolved { input, reason } => {
                println!("  {:<30} [{}] [skip]", input, reason);
            }
        }
    }
    println!(
        "Total: {} planned, {} not found, {} busy",
        counts.ready, counts.unresolved, counts.busy
    );
    if dry_run {
        println!("(dry-run; nothing deleted)");
    }
    println!();
}

/// Ask for a single y/N confirmation on the whole batch. Only invoked when
/// planned.ready > 1 (or planned.ready >= 1 combined with skips worth
/// surfacing). Returns true if the user confirmed.
pub fn confirm_batch() -> bool {
    if !(std::io::stdin().is_terminal() && std::io::stderr().is_terminal()) {
        return true; // non-interactive: assume confirmed (scripted usage)
    }
    eprint!("Proceed? (y/N): ");
    let _ = std::io::stderr().flush();
    let mut buf = String::new();
    if std::io::stdin().read_line(&mut buf).is_err() {
        return false;
    }
    let ans = buf.trim().to_lowercase();
    ans == "y" || ans == "yes"
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

    #[test]
    fn plan_busy_passes_unresolved_through_when_not_allowed() {
        let entries = vec![PlanEntry::Unresolved {
            input: "x".into(),
            reason: "not found".into(),
        }];
        let out = plan_busy(entries, false);
        assert_eq!(out.len(), 1);
        assert!(matches!(out[0], PlanEntry::Unresolved { .. }));
    }

    #[test]
    fn count_buckets_entries_correctly() {
        let entries = vec![
            PlanEntry::Ready(Resolved {
                input: "a".into(),
                path: PathBuf::from("/tmp/a"),
                branch: Some("a".into()),
            }),
            PlanEntry::Busy {
                resolved: Resolved {
                    input: "b".into(),
                    path: PathBuf::from("/tmp/b"),
                    branch: Some("b".into()),
                },
                info: vec![],
            },
            PlanEntry::Unresolved {
                input: "c".into(),
                reason: "not found".into(),
            },
        ];
        let c = count(&entries);
        assert_eq!(c.ready, 1);
        assert_eq!(c.busy, 1);
        assert_eq!(c.unresolved, 1);
    }
}
