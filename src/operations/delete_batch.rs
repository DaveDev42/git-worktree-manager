//! Batch deletion orchestration for `gw delete`.
//!
//! Multi-target deletion pipeline: resolve-all → plan (busy) → summary →
//! confirm → execute → exit code. Reuses `worktree::delete_one` for per-target
//! execution.

use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};

use console::style;

use crate::error::{CwError, Result};
use crate::git;
use crate::operations::busy::{self, BusyInfo};
use crate::operations::worktree::{self, DeleteFlags};

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

/// Final outcome used to compute exit code and summary.
///
/// Fields retain label/reason/error for Debug output and future per-item
/// reporting. The current summary only counts variants, so dead-code lint is
/// silenced here.
#[derive(Debug)]
#[allow(dead_code)]
enum ItemResult {
    Deleted(String),
    Skipped { label: String, reason: String },
    Failed { label: String, error: CwError },
}

fn label_of(entry: &PlanEntry) -> String {
    match entry {
        PlanEntry::Ready(r) => r.branch.clone().unwrap_or_else(|| r.input.clone()),
        PlanEntry::Busy { resolved, .. } => resolved
            .branch
            .clone()
            .unwrap_or_else(|| resolved.input.clone()),
        PlanEntry::Unresolved { input, .. } => input.clone(),
    }
}

/// Execute the plan sequentially. Best-effort: one failure does not abort.
fn execute_all(entries: Vec<PlanEntry>, flags: DeleteFlags) -> Result<Vec<ItemResult>> {
    let main_repo = git::get_main_repo_root(None)?;
    let mut results = Vec::with_capacity(entries.len());
    for entry in entries {
        let label = label_of(&entry);
        match entry {
            PlanEntry::Ready(r) => {
                // progress line → stdout
                println!("{} Deleting {}", style("•").cyan().bold(), label);
                match worktree::delete_one(&r.path, r.branch.as_deref(), &main_repo, flags) {
                    worktree::DeletionOutcome::Deleted { .. } => {
                        results.push(ItemResult::Deleted(label));
                    }
                    worktree::DeletionOutcome::Skipped { reason } => {
                        results.push(ItemResult::Skipped { label, reason });
                    }
                    worktree::DeletionOutcome::Failed { error } => {
                        // failure → stderr
                        eprintln!(
                            "{} Failed to delete {}: {}",
                            style("x").red().bold(),
                            label,
                            error
                        );
                        results.push(ItemResult::Failed { label, error });
                    }
                }
            }
            PlanEntry::Busy { .. } => {
                // skip notice → stdout (not an error, just an informational decision)
                println!("{} Skipped {} (busy)", style("~").yellow(), label);
                // Also mirror an error-level hint to stderr: busy skips
                // translate to a non-zero exit, so scripts watching stderr
                // should see *something*. Keeps parity with the legacy
                // single-target flow that errored with "in use" here.
                eprintln!(
                    "{} worktree '{}' is in use; re-run with --force to override",
                    style("error:").red().bold(),
                    label
                );
                results.push(ItemResult::Skipped {
                    label,
                    reason: "busy".into(),
                });
            }
            PlanEntry::Unresolved { input, reason } => {
                println!("{} Skipped {} ({})", style("~").yellow(), input, reason);
                results.push(ItemResult::Skipped {
                    label: input,
                    reason,
                });
            }
        }
    }
    Ok(results)
}

fn print_results(results: &[ItemResult]) {
    let deleted = results
        .iter()
        .filter(|r| matches!(r, ItemResult::Deleted(_)))
        .count();
    let skipped = results
        .iter()
        .filter(|r| matches!(r, ItemResult::Skipped { .. }))
        .count();
    let failed = results
        .iter()
        .filter(|r| matches!(r, ItemResult::Failed { .. }))
        .count();
    println!(
        "\nSummary: {} deleted, {} skipped, {} failed",
        deleted, skipped, failed
    );
}

fn exit_code_from(results: &[ItemResult]) -> i32 {
    for r in results {
        match r {
            ItemResult::Failed { .. } => return 2,
            ItemResult::Skipped { .. } => return 2,
            _ => {}
        }
    }
    0
}

/// If cwd lives inside any Ready/Busy target path, chdir to the main repo
/// root. Prevents the current `gw` process from being flagged as a busy holder
/// of the worktree it is being asked to remove.
fn move_cwd_out_of_targets(entries: &[PlanEntry]) {
    let Ok(cwd) = std::env::current_dir() else {
        return;
    };
    let cwd_canon = cwd.canonicalize().unwrap_or(cwd);
    for e in entries {
        let path = match e {
            PlanEntry::Ready(r) => &r.path,
            PlanEntry::Busy { resolved, .. } => &resolved.path,
            PlanEntry::Unresolved { .. } => continue,
        };
        let wt_canon = path.canonicalize().unwrap_or_else(|_| path.clone());
        if cwd_canon.starts_with(&wt_canon) {
            if let Ok(main_repo) = git::get_main_repo_root(None) {
                let _ = std::env::set_current_dir(&main_repo);
            }
            return;
        }
    }
}

/// Top-level orchestrator for `gw delete`.
///
/// `inputs` is empty for the legacy "current worktree" case and for the
/// `-i` interactive case — the caller passes `interactive=true` to trigger the
/// selector.
pub fn delete_worktrees(
    inputs: Vec<String>,
    interactive: bool,
    dry_run: bool,
    flags: DeleteFlags,
    lookup_mode: Option<&str>,
) -> Result<i32> {
    // 1) Decide the initial input set.
    let initial_inputs: Vec<String> = if interactive {
        // Filled in by Task 6 (TUI). Until then, reject explicitly.
        return Err(CwError::Other(
            "--interactive is not yet wired; coming in the next task".into(),
        ));
    } else if inputs.is_empty() {
        // Legacy path: delegate to the single-target shim and return its exit
        // code. Keeps the "no-args inside a worktree deletes current" behavior
        // and its busy prompt exactly as today.
        return legacy_single_current(flags, lookup_mode);
    } else {
        inputs
    };

    // 2) Resolve all inputs against the repo.
    let entries = resolve_all(&initial_inputs, lookup_mode)?;

    // 2.5) If cwd is inside any resolved target, move to the main repo *before*
    // busy detection so the running `gw` process doesn't register as a busy
    // holder of a worktree it's trying to delete. Mirrors the legacy
    // `delete_worktree` behavior.
    move_cwd_out_of_targets(&entries);

    // 3) Plan busy status.
    let entries = plan_busy(entries, flags.allow_busy);

    // 4) Print summary.
    print_summary(&entries, dry_run);

    // 5) Dry-run short-circuits before execution.
    if dry_run {
        return Ok(0);
    }

    // 6) Batch confirmation when the plan has more than one entry
    //    (Ready + Busy + Unresolved combined). Single-entry explicit
    //    positional goes straight to execute.
    if entries.len() >= 2 && !confirm_batch() {
        eprintln!("Cancelled.");
        return Ok(1);
    }

    // 7) Execute.
    let results = execute_all(entries, flags)?;
    print_results(&results);
    Ok(exit_code_from(&results))
}

fn legacy_single_current(flags: DeleteFlags, lookup_mode: Option<&str>) -> Result<i32> {
    match worktree::delete_worktree(
        None,
        flags.keep_branch,
        flags.delete_remote,
        flags.git_force,
        flags.allow_busy,
        lookup_mode,
    ) {
        Ok(()) => Ok(0),
        Err(e) => {
            eprintln!("{} {}", style("error:").red().bold(), e);
            Ok(2)
        }
    }
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
