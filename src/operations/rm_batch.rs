//! Batch removal orchestration for `gw rm`.
//!
//! Multi-target removal pipeline: resolve-all → plan (busy) → summary →
//! confirm → execute → exit code. Reuses `worktree::delete_one` for per-target
//! execution.

use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};

use console::style;

use crate::error::{CwError, Result};
use crate::git;
use crate::operations::busy::{self, BusyInfo};
use crate::operations::busy_messages;
use crate::operations::helpers;
use crate::operations::worktree::{self, RmFlags};

/// Result of the interactive multi-select flow.
///
/// - `Selected(v)` — user confirmed with at least one pick; `v` is non-empty.
/// - `Nothing` — no feature worktrees, or user confirmed with zero selections.
///   Nothing-to-do is not an error; the orchestrator exits 0.
/// - `Cancelled` — user pressed Esc / q / Ctrl-C. Orchestrator exits 1.
enum InteractiveOutcome {
    Selected(Vec<String>),
    Nothing,
    Cancelled,
}

/// Open the multi-select TUI to let the user choose which feature worktrees
/// to remove. Distinguishes Selected / Nothing / Cancelled so the caller can
/// map each to the exit code the spec requires.
fn interactive_select(main_repo: &Path) -> Result<InteractiveOutcome> {
    let feature_worktrees = git::get_feature_worktrees(Some(main_repo))?;
    if feature_worktrees.is_empty() {
        eprintln!("No feature worktrees to remove.");
        return Ok(InteractiveOutcome::Nothing);
    }
    let labels: Vec<String> = feature_worktrees
        .iter()
        .map(|(branch, path)| {
            let age = crate::constants::path_age_days(path)
                .map(crate::operations::display::format_age)
                .unwrap_or_default();
            let is_busy = !busy::detect_busy(path).is_empty();
            crate::operations::display::format_selector_row(
                branch,
                &age,
                is_busy,
                &path.display().to_string(),
                30,
            )
        })
        .collect();
    match crate::tui::multi_select::multi_select(&labels, "Select worktrees to remove:") {
        Some(indices) if indices.is_empty() => {
            eprintln!("Nothing selected.");
            Ok(InteractiveOutcome::Nothing)
        }
        Some(indices) => {
            let selected: Vec<String> = indices
                .into_iter()
                .map(|i| feature_worktrees[i].0.clone())
                .collect();
            Ok(InteractiveOutcome::Selected(selected))
        }
        None => {
            eprintln!("Cancelled.");
            Ok(InteractiveOutcome::Cancelled)
        }
    }
}

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
        hard: Vec<BusyInfo>,
        soft: Vec<BusyInfo>,
    },
    Unresolved {
        input: String,
        reason: String,
    },
}

/// Resolve a list of user inputs against the main repository.
///
/// Each input is resolved via strict ordered resolution:
/// exact worktree name → exact branch name → exact path.
/// Anything that does not match becomes a `PlanEntry::Unresolved`.
pub fn resolve_all(inputs: &[String]) -> Result<Vec<PlanEntry>> {
    let main_repo = git::get_main_repo_root(None)?;
    let mut out = Vec::with_capacity(inputs.len());
    for input in inputs {
        match resolve_one(input, &main_repo) {
            Some(resolved) => out.push(PlanEntry::Ready(resolved)),
            None => out.push(PlanEntry::Unresolved {
                input: input.clone(),
                reason: "not found".into(),
            }),
        }
    }
    Ok(out)
}

fn resolve_one(input: &str, main_repo: &Path) -> Option<Resolved> {
    match helpers::resolve_target_strict(main_repo, input) {
        Ok(strict) => Some(Resolved {
            input: input.to_string(),
            path: strict.path,
            branch: strict.branch,
        }),
        Err(_) => None,
    }
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
                let (hard, soft) = busy::detect_busy_tiered(&r.path);
                if hard.is_empty() && soft.is_empty() {
                    PlanEntry::Ready(r)
                } else {
                    PlanEntry::Busy {
                        resolved: r,
                        hard,
                        soft,
                    }
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

/// Print the batch summary. Summary/progress goes to stdout; errors and
/// prompts go to stderr.
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
            PlanEntry::Busy {
                resolved,
                hard,
                soft,
            } => {
                let label = resolved.branch.as_deref().unwrap_or(&resolved.input);
                let detail = hard
                    .first()
                    .or_else(|| soft.first())
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
fn execute_all(entries: Vec<PlanEntry>, flags: RmFlags) -> Result<Vec<ItemResult>> {
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
            PlanEntry::Busy { hard, soft, .. } => {
                // Summary line → stdout.
                println!("{} Skipped {} (busy)", style("~").yellow(), label);
                // Error mirror → stderr. Required so non-TTY `gw rm`
                // against a busy worktree emits a stderr hint matching the
                // legacy single-target flow (see tests/busy_detection.rs).
                eprint!(
                    "{} {}",
                    style("error:").red().bold(),
                    busy_messages::render_refusal(&label, &hard, &soft)
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
    let any_bad = results
        .iter()
        .any(|r| matches!(r, ItemResult::Failed { .. } | ItemResult::Skipped { .. }));
    if any_bad {
        2
    } else {
        0
    }
}

/// If cwd lives inside any Ready/Busy target path, chdir to the main repo
/// root. Prevents the current `gw` process from being flagged as a busy holder
/// of the worktree it is being asked to remove.
///
/// Canonicalize failures on either side are treated as "skip this comparison"
/// rather than falling back to the raw path. On filesystems with symlinked
/// tempdirs (e.g. `/var` -> `/private/var` on macOS) an asymmetric fallback
/// could mis-classify and leave cwd in the target.
fn move_cwd_out_of_targets(entries: &[PlanEntry]) {
    let Ok(cwd) = std::env::current_dir() else {
        return;
    };
    let Ok(cwd_canon) = cwd.canonicalize() else {
        return;
    };
    for e in entries {
        let path = match e {
            PlanEntry::Ready(r) => &r.path,
            PlanEntry::Busy { resolved, .. } => &resolved.path,
            PlanEntry::Unresolved { .. } => continue,
        };
        let Ok(wt_canon) = path.canonicalize() else {
            continue;
        };
        if cwd_canon.starts_with(&wt_canon) {
            if let Ok(main_repo) = git::get_main_repo_root(None) {
                let _ = std::env::set_current_dir(&main_repo);
            }
            return;
        }
    }
}

/// Top-level orchestrator for `gw rm`.
///
/// `inputs` is empty for the legacy "current worktree" case and for the
/// `-i` interactive case — the caller passes `interactive=true` to trigger the
/// selector.
pub fn rm_worktrees(
    inputs: Vec<String>,
    interactive: bool,
    dry_run: bool,
    flags: RmFlags,
) -> Result<i32> {
    // 1) Decide the initial input set.
    let initial_inputs: Vec<String> = if interactive {
        debug_assert!(
            inputs.is_empty(),
            "clap should have rejected -i with positionals"
        );
        let main_repo = git::get_main_repo_root(None)?;
        match interactive_select(&main_repo)? {
            InteractiveOutcome::Selected(v) => v,
            InteractiveOutcome::Nothing => return Ok(0),
            InteractiveOutcome::Cancelled => return Ok(1),
        }
    } else if inputs.is_empty() {
        // Legacy path: delegate to the single-target shim and return its exit
        // code. Keeps the "no-args inside a worktree deletes current" behavior
        // and its busy prompt exactly as today.
        return legacy_single_current(flags);
    } else {
        inputs
    };

    // 2) Resolve all inputs against the repo.
    let entries = resolve_all(&initial_inputs)?;

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

fn legacy_single_current(flags: RmFlags) -> Result<i32> {
    match worktree::delete_worktree(
        None,
        flags.keep_branch,
        flags.delete_remote,
        flags.git_force,
        flags.allow_busy,
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
                hard: vec![],
                soft: vec![],
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
