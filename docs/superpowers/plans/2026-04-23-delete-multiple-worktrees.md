# Delete Multiple Worktrees Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend `gw delete` to accept multiple targets via positional args or a new multi-select TUI, with batch confirmation, best-effort execution, and non-zero exit on partial failure — without changing single-target behavior.

**Architecture:** Refactor `delete_worktree()` into a private per-target `delete_one()` plus a new `delete_worktrees()` orchestrator that handles selection (positional / interactive / legacy current-worktree), resolve-all-first, busy planning, a single batch confirmation, sequential execution, and exit code aggregation. Add a new arrow/checkbox multi-select TUI (Unix raw-mode + line-based fallback) that emits a `Vec<Selected>` fed into the same orchestrator.

**Tech Stack:** Rust, clap derive CLI, `console` crate for styling, stdlib raw-mode via existing `src/tui/arrow_select.rs` pattern, `std::process::Command` for git operations.

**Spec:** `docs/superpowers/specs/2026-04-23-delete-multiple-worktrees-design.md`

---

## File Structure

Files to create:
- `src/tui/multi_select.rs` — new multi-select widget (Unix raw-mode + line fallback).
- `src/operations/delete_batch.rs` — new orchestrator module (`delete_worktrees`, plan types, summary/confirmation/reporting helpers).
- `tests/test_delete_multi.rs` — new integration tests for multi-target delete.

Files to modify:
- `src/cli.rs` — `Commands::Delete` shape: `targets: Vec<String>`, `interactive`, `dry_run`.
- `src/entrypoint.rs` — route `Commands::Delete` to `delete_batch::delete_worktrees(...)`.
- `src/operations/mod.rs` — declare new `delete_batch` module.
- `src/operations/worktree.rs` — split `delete_worktree` into `delete_one` (per-target) + keep the current public `delete_worktree` as a thin shim for legacy `clean` callers.
- `src/operations/clean.rs` — still calls `worktree::delete_worktree` (unchanged signature for legacy path). Confirm no breakage.
- `src/tui/mod.rs` — expose `multi_select` module.
- `tests/test_cli.rs` — extend delete help / parse tests.

---

## Ground rules for every task

- **Run `cargo fmt` and `cargo clippy` before every commit.** Zero clippy warnings policy.
- **No `unwrap()` in production code.** Use `Result<T>` with `CwError`.
- **Use conventional commits.** Default prefix: `feat:` or `refactor:` or `test:`. Never `feat!` / `BREAKING CHANGE:` — the repo enforces patch-only bumps (see `CLAUDE.md`).
- **Each task ends with a commit** that leaves the tree green (`cargo test` passes).

---

## Task 1: CLI surface change — accept `Vec<String>` + `--interactive` + `--dry-run`

**Files:**
- Modify: `src/cli.rs` (the `Delete` variant and nearby tests)
- Modify: `tests/test_cli.rs`

- [ ] **Step 1: Write a failing CLI parse test**

Append to `tests/test_cli.rs` (pick an appropriate location next to other delete tests):

```rust
#[test]
fn test_delete_accepts_multiple_targets() {
    use git_worktree_manager::cli::{Cli, Commands};
    use clap::Parser;
    let cli = Cli::try_parse_from(["gw", "delete", "feat/a", "feat/b", "feat/c"])
        .expect("parses");
    let Some(Commands::Delete { targets, interactive, dry_run, .. }) = cli.command else {
        panic!("expected Delete, got {:?}", cli.command);
    };
    assert_eq!(targets, vec!["feat/a", "feat/b", "feat/c"]);
    assert!(!interactive);
    assert!(!dry_run);
}

#[test]
fn test_delete_interactive_flag_parses() {
    use git_worktree_manager::cli::{Cli, Commands};
    use clap::Parser;
    let cli = Cli::try_parse_from(["gw", "delete", "-i"]).expect("parses");
    let Some(Commands::Delete { targets, interactive, .. }) = cli.command else {
        panic!("expected Delete");
    };
    assert!(targets.is_empty());
    assert!(interactive);
}

#[test]
fn test_delete_dry_run_flag_parses() {
    use git_worktree_manager::cli::{Cli, Commands};
    use clap::Parser;
    let cli = Cli::try_parse_from(["gw", "delete", "a", "--dry-run"]).expect("parses");
    let Some(Commands::Delete { targets, dry_run, .. }) = cli.command else {
        panic!("expected Delete");
    };
    assert_eq!(targets, vec!["a"]);
    assert!(dry_run);
}

#[test]
fn test_delete_interactive_conflicts_with_positional() {
    use git_worktree_manager::cli::Cli;
    use clap::Parser;
    let err = Cli::try_parse_from(["gw", "delete", "-i", "a"]).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("cannot be used") || msg.contains("conflict"),
        "expected conflict error, got: {msg}"
    );
}
```

- [ ] **Step 2: Run failing tests**

Run: `cargo test -p git-worktree-manager --test test_cli -- test_delete_accepts_multiple_targets test_delete_interactive_flag_parses test_delete_dry_run_flag_parses test_delete_interactive_conflicts_with_positional`

Expected: all four fail (the `Delete` variant still has `target: Option<String>` and no `interactive`/`dry_run`).

- [ ] **Step 3: Update the `Delete` variant in `src/cli.rs`**

Replace the existing `Delete { target: Option<String>, ... }` block (around `src/cli.rs:214-243`) with:

```rust
    /// Delete one or more worktrees
    Delete {
        /// Branch names or paths of worktrees to delete.
        /// If empty and --interactive is not set, deletes the current worktree.
        #[arg(conflicts_with = "interactive")]
        targets: Vec<String>,

        /// Interactive multi-select UI (mutually exclusive with positional targets)
        #[arg(short, long, conflicts_with = "targets")]
        interactive: bool,

        /// Show what would be deleted without deleting
        #[arg(long)]
        dry_run: bool,

        /// Keep the branch (only remove worktree)
        #[arg(short = 'k', long)]
        keep_branch: bool,

        /// Also delete the remote branch
        #[arg(short = 'r', long)]
        delete_remote: bool,

        /// Force remove: also bypasses the busy-detection gate (skips the
        /// "worktree is in use" check and deletes anyway)
        #[arg(short, long, conflicts_with = "no_force")]
        force: bool,

        /// Don't use --force flag
        #[arg(long)]
        no_force: bool,

        /// Resolve targets as worktree names (instead of branches)
        #[arg(short, long)]
        worktree: bool,

        /// Resolve targets as branch names (instead of worktrees)
        #[arg(short, long, conflicts_with = "worktree")]
        branch: bool,
    },
```

- [ ] **Step 4: Re-run the CLI tests**

Run: `cargo test -p git-worktree-manager --test test_cli`
Expected: new tests pass. The build will now fail in `src/entrypoint.rs` because the `Delete` pattern no longer matches — that is expected; it is fixed in Task 2.

- [ ] **Step 5: Fix the entrypoint pattern temporarily (to keep the tree building)**

Open `src/entrypoint.rs` and locate the `Some(Commands::Delete { target, ... })` match arm (around line 185). Replace it temporarily with a stub that preserves legacy single-target behavior:

```rust
        Some(Commands::Delete {
            targets,
            interactive: _,
            dry_run: _,
            keep_branch,
            delete_remote,
            force,
            no_force,
            worktree: is_worktree,
            branch: is_branch,
        }) => {
            let lookup_mode = resolve_lookup_mode(is_worktree, is_branch);
            let single = targets.into_iter().next();
            // Temporary single-target shim; replaced by Task 2 orchestrator.
            worktree::delete_worktree(
                single.as_deref(),
                keep_branch,
                delete_remote,
                !no_force,
                force,
                lookup_mode,
            )
        }
```

This keeps the project compiling and existing single-target tests passing while Task 2 builds the orchestrator.

- [ ] **Step 6: Verify everything still builds and existing tests pass**

Run: `cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test`
Expected: clean build, no clippy warnings, all existing tests + the four new CLI tests pass. Multi-target runtime behavior is NOT yet implemented; that is Task 2+.

- [ ] **Step 7: Commit**

```bash
git add src/cli.rs src/entrypoint.rs tests/test_cli.rs
git commit -m "feat(cli): accept multiple delete targets and -i/--dry-run flags"
```

---

## Task 2: Extract per-target `delete_one()` and keep the legacy public API

**Goal:** Split today's `delete_worktree()` into two layers without behavior change yet:
- `delete_one()` — private, takes an already-resolved target and executes the deletion. No prompts, no summaries.
- `delete_worktree()` — remains public with the same signature and semantics for existing callers (`clean.rs`, test suite, and the Task 1 shim). It resolves, handles the legacy busy prompt, then calls `delete_one()`.

**Files:**
- Modify: `src/operations/worktree.rs`

- [ ] **Step 1: Read the current implementation carefully**

Open `src/operations/worktree.rs:218-380` (current `delete_worktree`) and `src/operations/worktree.rs:382-422` (current `resolve_delete_target`). Confirm the sequence: resolve → main-repo safety → cwd move → busy gate → pre-hook → git remove → branch + metadata + optional remote push → post-hook → registry update.

- [ ] **Step 2: Add a `DeletionOutcome` type and `delete_one()` private function**

Inside `src/operations/worktree.rs`, add near the top of the delete section:

```rust
/// Outcome of attempting to delete a single worktree.
#[derive(Debug)]
pub enum DeletionOutcome {
    Deleted { branch: Option<String>, path: PathBuf },
    Skipped { reason: String },
    Failed { error: CwError },
}

/// Flags that apply uniformly to every target in a batch.
#[derive(Debug, Clone, Copy)]
pub struct DeleteFlags {
    pub keep_branch: bool,
    pub delete_remote: bool,
    /// Passes through to `git worktree remove --force` (historical semantic).
    pub git_force: bool,
    /// Bypass the busy-detection gate.
    pub allow_busy: bool,
}

/// Per-target deletion. Assumes the caller has already resolved the target
/// and decided to proceed (no summary, no batch confirmation, no busy prompt
/// — the orchestrator handles those).
///
/// Returns an outcome describing what happened. Never prints a batch summary;
/// individual progress lines are acceptable.
pub(crate) fn delete_one(
    worktree_path: &Path,
    branch_name: Option<&str>,
    main_repo: &Path,
    flags: DeleteFlags,
) -> DeletionOutcome {
    // Safety: never delete the main worktree.
    let wt_resolved = git::canonicalize_or(worktree_path);
    let main_resolved = git::canonicalize_or(main_repo);
    if wt_resolved == main_resolved {
        return DeletionOutcome::Failed {
            error: CwError::Git(messages::cannot_delete_main_worktree()),
        };
    }

    // If cwd is inside worktree, move to main_repo before deletion.
    if let Ok(cwd) = std::env::current_dir() {
        let cwd_canon = cwd.canonicalize().unwrap_or(cwd);
        let wt_canon = worktree_path
            .canonicalize()
            .unwrap_or_else(|_| worktree_path.to_path_buf());
        if cwd_canon.starts_with(&wt_canon) {
            let _ = std::env::set_current_dir(main_repo);
        }
    }

    // Pre-delete hook
    let base_branch = branch_name
        .and_then(|b| {
            let key = format_config_key(CONFIG_KEY_BASE_BRANCH, b);
            git::get_config(&key, Some(main_repo))
        })
        .unwrap_or_default();

    let mut hook_ctx = build_hook_context(
        branch_name.unwrap_or(""),
        &base_branch,
        worktree_path,
        main_repo,
        "worktree.pre_delete",
        "delete",
    );
    if let Err(e) = hooks::run_hooks(
        "worktree.pre_delete",
        &hook_ctx,
        Some(main_repo),
        Some(main_repo),
    ) {
        return DeletionOutcome::Failed { error: e };
    }

    // Remove worktree
    println!(
        "{}",
        style(messages::removing_worktree(worktree_path)).yellow()
    );
    if let Err(e) = git::remove_worktree_safe(worktree_path, main_repo, flags.git_force) {
        return DeletionOutcome::Failed { error: e };
    }
    println!("{} Worktree removed\n", style("*").green().bold());

    // Delete branch + metadata + optional remote push
    if let Some(branch) = branch_name {
        if !flags.keep_branch {
            println!("{}", style(messages::deleting_local_branch(branch)).yellow());
            let _ = git::git_command(&["branch", "-D", branch], Some(main_repo), false, false);

            let bb_key = format_config_key(CONFIG_KEY_BASE_BRANCH, branch);
            let bp_key = format_config_key(CONFIG_KEY_BASE_PATH, branch);
            let ib_key = format_config_key(CONFIG_KEY_INTENDED_BRANCH, branch);
            git::unset_config(&bb_key, Some(main_repo));
            git::unset_config(&bp_key, Some(main_repo));
            git::unset_config(&ib_key, Some(main_repo));

            println!(
                "{} Local branch and metadata removed\n",
                style("*").green().bold()
            );

            if flags.delete_remote {
                println!(
                    "{}",
                    style(messages::deleting_remote_branch(branch)).yellow()
                );
                match git::git_command(
                    &["push", "origin", &format!(":{}", branch)],
                    Some(main_repo),
                    false,
                    true,
                ) {
                    Ok(r) if r.returncode == 0 => {
                        println!("{} Remote branch deleted\n", style("*").green().bold());
                    }
                    _ => {
                        println!("{} Remote branch deletion failed\n", style("!").yellow());
                    }
                }
            }
        }
    }

    // Post-delete hook
    hook_ctx.insert("event".into(), "worktree.post_delete".into());
    let _ = hooks::run_hooks(
        "worktree.post_delete",
        &hook_ctx,
        Some(main_repo),
        Some(main_repo),
    );
    let _ = registry::update_last_seen(main_repo);

    DeletionOutcome::Deleted {
        branch: branch_name.map(str::to_string),
        path: worktree_path.to_path_buf(),
    }
}
```

- [ ] **Step 3: Rewrite `delete_worktree()` as a thin shim calling `delete_one()`**

Replace the body of the existing `pub fn delete_worktree(...)` with:

```rust
pub fn delete_worktree(
    target: Option<&str>,
    keep_branch: bool,
    delete_remote: bool,
    force: bool,
    allow_busy: bool,
    lookup_mode: Option<&str>,
) -> Result<()> {
    let main_repo = git::get_main_repo_root(None)?;
    let (worktree_path, branch_name) = resolve_delete_target(target, &main_repo, lookup_mode)?;

    // Main-repo safety guard (mirrors delete_one, but we want the error surfaced
    // up before prompting).
    let wt_resolved = git::canonicalize_or(&worktree_path);
    let main_resolved = git::canonicalize_or(&main_repo);
    if wt_resolved == main_resolved {
        return Err(CwError::Git(messages::cannot_delete_main_worktree()));
    }

    // Legacy single-target busy prompt (unchanged behavior).
    let busy = crate::operations::busy::detect_busy(&worktree_path);
    if !busy.is_empty() && !allow_busy {
        let branch_display = branch_name.clone().unwrap_or_else(|| {
            worktree_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| worktree_path.to_string_lossy().to_string())
        });
        eprintln!(
            "{} worktree '{}' is in use by:",
            style("error:").red().bold(),
            branch_display
        );
        for b in &busy {
            eprintln!("    PID {:>6}  {}  (source: {:?})", b.pid, b.cmd, b.source);
        }

        use std::io::IsTerminal;
        if std::io::stdin().is_terminal() && std::io::stderr().is_terminal() {
            use std::io::Write;
            eprint!("Delete anyway? (y/N): ");
            let _ = std::io::stderr().flush();
            let mut buf = String::new();
            std::io::stdin().read_line(&mut buf)?;
            let ans = buf.trim().to_lowercase();
            if ans != "y" && ans != "yes" {
                eprintln!("Aborted.");
                return Ok(());
            }
        } else {
            return Err(CwError::Other(format!(
                "worktree '{}' is in use by {} process(es); re-run with --force to override",
                branch_display,
                busy.len()
            )));
        }
    }

    let flags = DeleteFlags {
        keep_branch,
        delete_remote,
        git_force: force,
        allow_busy: true, // already gated above
    };

    match delete_one(&worktree_path, branch_name.as_deref(), &main_repo, flags) {
        DeletionOutcome::Deleted { .. } => Ok(()),
        DeletionOutcome::Skipped { reason } => Err(CwError::Other(reason)),
        DeletionOutcome::Failed { error } => Err(error),
    }
}
```

- [ ] **Step 4: Run the full test suite**

Run: `cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test`
Expected: all tests pass — this is a refactor, behavior unchanged. Pay particular attention to `test_operations::test_delete_*` and `test_workflows::test_full_workflow_new_list_delete`.

- [ ] **Step 5: Commit**

```bash
git add src/operations/worktree.rs
git commit -m "refactor(worktree): extract delete_one for per-target deletion"
```

---

## Task 3: Plan types and resolver for batch deletion

**Goal:** Create `src/operations/delete_batch.rs` with the data types (`TargetInput`, `Resolved`, `PlanEntry`) and the resolve-all + plan helpers. No orchestration yet.

**Files:**
- Create: `src/operations/delete_batch.rs`
- Modify: `src/operations/mod.rs`

- [ ] **Step 1: Create the module file with types and a resolver function**

Create `src/operations/delete_batch.rs`:

```rust
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
    Busy { resolved: Resolved, info: Vec<BusyInfo> },
    Unresolved { input: String, reason: String },
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

fn resolve_one(
    input: &str,
    main_repo: &Path,
    lookup_mode: Option<&str>,
) -> Option<Resolved> {
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
```

- [ ] **Step 2: Register the new module**

Open `src/operations/mod.rs` and add `pub mod delete_batch;` in alphabetical order with the other `pub mod` declarations. Confirm via `cargo build` that the module is picked up.

- [ ] **Step 3: Run tests**

Run: `cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test`
Expected: the new unit test `plan_busy_passthrough_when_allowed` passes. Everything else still passes.

- [ ] **Step 4: Commit**

```bash
git add src/operations/delete_batch.rs src/operations/mod.rs
git commit -m "feat(delete_batch): add plan types and resolver"
```

---

## Task 4: Summary printing, dry-run path, batch confirmation prompt

**Goal:** Add the print/confirmation helpers to `delete_batch.rs`. No execution yet.

**Files:**
- Modify: `src/operations/delete_batch.rs`

- [ ] **Step 1: Add helpers for summary + confirmation**

Append to `src/operations/delete_batch.rs`:

```rust
use console::style;
use std::io::{IsTerminal, Write};

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

/// Print the batch summary to stderr. Used both for dry-run and real runs.
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
    eprintln!("\n{}", style(header).yellow().bold());
    for e in entries {
        match e {
            PlanEntry::Ready(r) => {
                let label = r.branch.as_deref().unwrap_or(&r.input);
                eprintln!("  {:<30} {}", label, r.path.display());
            }
            PlanEntry::Busy { resolved, info } => {
                let label = resolved.branch.as_deref().unwrap_or(&resolved.input);
                let detail = info
                    .first()
                    .map(|b| format!("PID {} {}", b.pid, b.cmd))
                    .unwrap_or_default();
                eprintln!("  {:<30} (busy: {})  [skip]", label, detail);
            }
            PlanEntry::Unresolved { input, reason } => {
                eprintln!("  {:<30} [{}] [skip]", input, reason);
            }
        }
    }
    eprintln!(
        "Total: {} planned, {} not found, {} busy\n",
        counts.ready, counts.unresolved, counts.busy
    );
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
mod tests_summary {
    use super::*;

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
```

**Note on the non-interactive confirmation policy:** when the batch runs under
a non-TTY stdin (CI, scripts, test harness using `assert_cmd`), we treat that
as implicit consent and proceed. This matches how the existing single-target
busy prompt behaves (it errors out rather than hanging) but is more permissive
because the batch prompt is a *new* safety net — breaking non-interactive
scripts that relied on `gw delete` completing without confirmation would be a
regression. `--dry-run` remains the scripted-preview mechanism.

- [ ] **Step 2: Run tests**

Run: `cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test`
Expected: pass, including the new `count_buckets_entries_correctly` test.

- [ ] **Step 3: Commit**

```bash
git add src/operations/delete_batch.rs
git commit -m "feat(delete_batch): add summary printer and batch confirmation"
```

---

## Task 5: Execute phase + exit code aggregation

**Goal:** Add `execute_all()` and a top-level `delete_worktrees()` orchestrator. Wire everything but the TUI (`-i` uses a stub that errors for now).

**Files:**
- Modify: `src/operations/delete_batch.rs`
- Modify: `src/entrypoint.rs`

- [ ] **Step 1: Add `execute_all` and `delete_worktrees` to `delete_batch.rs`**

Append to `src/operations/delete_batch.rs`:

```rust
use crate::error::CwError;
use crate::operations::worktree::{self, DeleteFlags};

/// Final outcome used to compute exit code and summary.
#[derive(Debug)]
enum ItemResult {
    Deleted(String),
    Skipped { label: String, reason: String },
    Failed { label: String, error: CwError },
}

fn label_of(entry: &PlanEntry) -> String {
    match entry {
        PlanEntry::Ready(r) => r.branch.clone().unwrap_or_else(|| r.input.clone()),
        PlanEntry::Busy { resolved, .. } => {
            resolved.branch.clone().unwrap_or_else(|| resolved.input.clone())
        }
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
                eprintln!("{} Deleting {}", style("•").cyan().bold(), label);
                match worktree::delete_one(&r.path, r.branch.as_deref(), &main_repo, flags) {
                    worktree::DeletionOutcome::Deleted { .. } => {
                        results.push(ItemResult::Deleted(label));
                    }
                    worktree::DeletionOutcome::Skipped { reason } => {
                        results.push(ItemResult::Skipped { label, reason });
                    }
                    worktree::DeletionOutcome::Failed { error } => {
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
                eprintln!("{} Skipped {} (busy)", style("~").yellow(), label);
                results.push(ItemResult::Skipped {
                    label,
                    reason: "busy".into(),
                });
            }
            PlanEntry::Unresolved { input, reason } => {
                eprintln!("{} Skipped {} ({})", style("~").yellow(), input, reason);
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
    let deleted = results.iter().filter(|r| matches!(r, ItemResult::Deleted(_))).count();
    let skipped = results.iter().filter(|r| matches!(r, ItemResult::Skipped { .. })).count();
    let failed = results.iter().filter(|r| matches!(r, ItemResult::Failed { .. })).count();
    eprintln!(
        "\nSummary: {} deleted, {} skipped, {} failed",
        deleted, skipped, failed
    );
}

fn exit_code_from(results: &[ItemResult]) -> i32 {
    for r in results {
        match r {
            ItemResult::Failed { .. } => return 2,
            ItemResult::Skipped { reason, .. }
                if reason == "busy" || reason == "not found" =>
            {
                return 2;
            }
            ItemResult::Skipped { .. } => return 2,
            _ => {}
        }
    }
    0
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

    // 3) Plan busy status.
    let entries = plan_busy(entries, flags.allow_busy);

    // 4) Print summary.
    print_summary(&entries, dry_run);

    // 5) Dry-run short-circuits before execution.
    if dry_run {
        return Ok(0);
    }

    // 6) Batch confirmation when more than one target is in the plan
    //    (Ready + Busy + Unresolved combined). For a single-entry plan, the
    //    legacy prompt path already handles it (or we just execute directly
    //    for a one-off explicit positional).
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
```

- [ ] **Step 2: Replace the temporary shim in `src/entrypoint.rs` with the orchestrator**

Replace the `Some(Commands::Delete { ... })` arm added in Task 1 with:

```rust
        Some(Commands::Delete {
            targets,
            interactive,
            dry_run,
            keep_branch,
            delete_remote,
            force,
            no_force,
            worktree: is_worktree,
            branch: is_branch,
        }) => {
            let lookup_mode = resolve_lookup_mode(is_worktree, is_branch);
            let flags = crate::operations::worktree::DeleteFlags {
                keep_branch,
                delete_remote,
                git_force: !no_force,
                allow_busy: force,
            };
            match crate::operations::delete_batch::delete_worktrees(
                targets,
                interactive,
                dry_run,
                flags,
                lookup_mode,
            ) {
                Ok(0) => Ok(()),
                Ok(code) => Err(crate::error::CwError::ExitCode(code)),
                Err(e) => Err(e),
            }
        }
```

- [ ] **Step 3: Add a new `ExitCode(i32)` variant to `CwError`**

Open `src/error.rs`, locate the `CwError` enum, and add:

```rust
    /// Terminate the process with a specific non-zero exit code without
    /// printing an error message. Used when the orchestrator has already
    /// produced a summary.
    #[error("")]
    ExitCode(i32),
```

Then in `main.rs` (or wherever `CwError` is mapped to a process exit), map `ExitCode(n)` → `std::process::exit(n)` without printing. If that mapping already exists generically, confirm it suppresses the Display output for the empty-message case. Otherwise add a dedicated branch at the top of the error-to-exit handler.

Search command to find the exit mapping:

```bash
grep -rn "std::process::exit\|CwError::" src/bin src/entrypoint.rs src/lib.rs | head
```

Wire a branch like:

```rust
Err(CwError::ExitCode(code)) => std::process::exit(code),
```

- [ ] **Step 4: Run all tests**

Run: `cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test`
Expected: all existing tests still pass. Multi-target runtime now works for positional invocations; `-i` still errors with "not yet wired".

- [ ] **Step 5: Smoke test multi-target in a throwaway repo (manual)**

```bash
cd /tmp && rm -rf smoke && mkdir smoke && cd smoke && git init && echo hi > r.md && git add r.md && git commit -m init
cargo run -q --manifest-path /Users/dave/Projects/github.com/git-worktree-manager-feat-cw-delete-multiple-worktree/Cargo.toml -- new a --no-term
cargo run -q --manifest-path /Users/dave/Projects/github.com/git-worktree-manager-feat-cw-delete-multiple-worktree/Cargo.toml -- new b --no-term
cargo run -q --manifest-path /Users/dave/Projects/github.com/git-worktree-manager-feat-cw-delete-multiple-worktree/Cargo.toml -- delete a b --dry-run
```

Expected: the dry-run summary lists both `a` and `b` as planned, says "(dry-run; nothing deleted)" (or equivalent), and exits 0. Then run without `--dry-run`:

```bash
cargo run -q --manifest-path /Users/dave/Projects/github.com/git-worktree-manager-feat-cw-delete-multiple-worktree/Cargo.toml -- delete a b
```

Expected: prompt "Proceed? (y/N):", answer `y`, both worktrees removed. `gw list` shows neither.

- [ ] **Step 6: Commit**

```bash
git add src/operations/delete_batch.rs src/entrypoint.rs src/error.rs
git commit -m "feat(delete): orchestrate multi-target delete with dry-run and batch confirm"
```

---

## Task 6: Multi-select TUI widget

**Goal:** Add a minimal multi-select arrow/checkbox TUI under `src/tui/multi_select.rs`, mirroring the raw-mode pattern used by `arrow_select.rs`. Integrate into `delete_worktrees` for the `-i` path.

**Files:**
- Create: `src/tui/multi_select.rs`
- Modify: `src/tui/arrow_select.rs` (expose a few helpers as `pub(crate)`)
- Modify: `src/tui/mod.rs`
- Modify: `src/operations/delete_batch.rs`

### Strategy: share raw-mode plumbing via `pub(crate)` helpers

The existing `src/tui/arrow_select.rs` already contains the Unix raw-mode
plumbing we need: `get_terminal_width`, `write_stderr`, `visible_len`,
`truncate`, `cleanup`, and a `Key` enum with `read_key(fd)`. We will:

1. Expose them as `pub(crate)` so `multi_select.rs` can import them.
2. Add a new `Key::Space` variant (currently Space is not in `Key`).
3. Build `multi_select.rs` on top of these shared helpers.

- [ ] **Step 1: Expose helpers in `arrow_select.rs`**

In `src/tui/arrow_select.rs`, change the visibility of the following items
from private to `pub(crate)`:

- `fn get_terminal_width` (line ~52)
- `fn write_stderr` (line ~58)
- `fn visible_len` (line ~67)
- `fn truncate` (line ~91)
- `enum Key` (line ~188)  *and its variants*
- `fn read_key` (line ~201)
- `fn cleanup` (line ~172) — rename exposure: keep implementation, mark `pub(crate)`.

Also extend `Key` with:

```rust
#[cfg(unix)]
#[derive(Debug, PartialEq)]
pub(crate) enum Key {
    Up,
    Down,
    Enter,
    Escape,
    CtrlC,
    Quit,
    Space,
    Number(u8),
    Unknown,
}
```

and add the space case in `read_key`:

```rust
        b' ' => Ok(Key::Space),
```

(place it next to the other `b'q'` / `b'1'..=b'9'` arms).

- [ ] **Step 2: Sanity-check the existing tests still pass**

Run: `cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test`
Expected: visibility changes are backwards compatible; everything passes.

- [ ] **Step 3: Commit the exposure change separately**

```bash
git add src/tui/arrow_select.rs
git commit -m "refactor(tui): expose arrow_select raw-mode helpers to the crate"
```

- [ ] **Step 4: Write the multi-select module**

Create `src/tui/multi_select.rs`. This mirrors `arrow_select_unix` but tracks
a `Vec<bool>` of selections, adds `Space` handling, and returns a vector of
indices. Raw-mode setup (tcgetattr/cfmakeraw/tcsetattr) is copied inline from
`arrow_select.rs`'s `arrow_select_unix` — it's ~15 lines and duplicating is
fine for two call sites (we'll extract to a shared helper if a third widget
appears).

```rust
//! Arrow/checkbox multi-select TUI used by `gw delete -i`.
//!
//! Built on the raw-mode plumbing shared from `arrow_select.rs`.

use std::io::IsTerminal;

use super::arrow_select::{get_terminal_width, read_key, truncate, write_stderr, Key};

/// Multi-select entry point. Returns selected indices in ascending order,
/// or `None` if the user cancelled. An empty Vec means the user confirmed
/// with zero selections.
pub fn multi_select(items: &[String], title: &str) -> Option<Vec<usize>> {
    if items.is_empty() {
        return Some(Vec::new());
    }
    if !std::io::stderr().is_terminal() {
        return multi_select_fallback(items, title);
    }

    #[cfg(unix)]
    {
        if let Some(result) = multi_select_unix(items, title) {
            return result;
        }
    }

    multi_select_fallback(items, title)
}

// -- Unix raw-mode --------------------------------------------------------

#[cfg(unix)]
fn multi_select_unix(items: &[String], title: &str) -> Option<Option<Vec<usize>>> {
    use std::os::unix::io::AsRawFd;

    let stdin = std::io::stdin();
    let fd = stdin.as_raw_fd();

    // Save original terminal attributes
    let mut old_termios: libc::termios = unsafe { std::mem::zeroed() };
    if unsafe { libc::tcgetattr(fd, &mut old_termios) } != 0 {
        return None;
    }

    // Enter raw mode
    let mut raw = old_termios;
    unsafe { libc::cfmakeraw(&mut raw) };
    if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &raw) } != 0 {
        return None;
    }

    // Hide cursor
    write_stderr("\x1b[?25l");

    let mut cursor = 0usize;
    let mut checked: Vec<bool> = vec![false; items.len()];
    let total_lines = items.len() + 3; // title + blank + items + hint

    render(items, &checked, cursor, title, true);

    let result: Option<Vec<usize>> = loop {
        match read_key(fd) {
            Ok(Key::Up) => {
                if cursor > 0 { cursor -= 1; }
                render(items, &checked, cursor, title, false);
            }
            Ok(Key::Down) => {
                if cursor + 1 < items.len() { cursor += 1; }
                render(items, &checked, cursor, title, false);
            }
            Ok(Key::Space) => {
                checked[cursor] = !checked[cursor];
                render(items, &checked, cursor, title, false);
            }
            Ok(Key::Enter) => {
                break Some(
                    checked
                        .iter()
                        .enumerate()
                        .filter_map(|(i, &c)| if c { Some(i) } else { None })
                        .collect(),
                );
            }
            Ok(Key::Escape) | Ok(Key::Quit) | Ok(Key::CtrlC) | Err(_) => {
                break None;
            }
            _ => {}
        }
    };

    // Cleanup: show cursor, restore termios, clear our drawn lines
    write_stderr("\x1b[?25h");
    super::arrow_select::cleanup(total_lines);
    unsafe { libc::tcsetattr(fd, libc::TCSANOW, &old_termios) };

    Some(result)
}

#[cfg(unix)]
fn render(items: &[String], checked: &[bool], cursor: usize, title: &str, first: bool) {
    let width = get_terminal_width();

    if !first {
        write_stderr("\x1b[u");
    }
    write_stderr("\x1b[s");

    // Title
    let line = format!("  \x1b[1m{title}\x1b[0m");
    write_stderr(&format!("\x1b[2K{}\r\n", truncate(&line, width)));
    write_stderr("\x1b[2K\r\n");

    for (i, label) in items.iter().enumerate() {
        write_stderr("\x1b[2K");
        let mark = if checked[i] { "[x]" } else { "[ ]" };
        let line = if i == cursor {
            format!("  \x1b[1;7m > {mark} {label} \x1b[0m")
        } else {
            format!("    {mark} {label}")
        };
        write_stderr(&format!("{}\r\n", truncate(&line, width)));
    }

    // Hint line
    write_stderr("\x1b[2K");
    write_stderr("  \x1b[2m(Space: toggle, Enter: confirm, Esc/q: cancel)\x1b[0m\r\n");

    // Blank spacer
    write_stderr("\x1b[2K\r\n");
    // Move cursor back up above the trailing blank
    write_stderr("\x1b[2A");
}

// -- Fallback (non-Unix or non-TTY) --------------------------------------

fn multi_select_fallback(items: &[String], title: &str) -> Option<Vec<usize>> {
    eprintln!("{}", title);
    for (i, item) in items.iter().enumerate() {
        eprintln!("  [{}] {}", i + 1, item);
    }
    eprintln!("Enter numbers (space- or comma-separated), 'all', or blank to cancel:");
    let mut buf = String::new();
    if std::io::stdin().read_line(&mut buf).is_err() {
        return None;
    }
    let s = buf.trim();
    if s.is_empty() {
        return None;
    }
    if s.eq_ignore_ascii_case("all") {
        return Some((0..items.len()).collect());
    }
    let mut out = Vec::new();
    for part in s.split(|c: char| c == ',' || c.is_whitespace()) {
        if part.is_empty() {
            continue;
        }
        if let Ok(n) = part.parse::<usize>() {
            if n >= 1 && n <= items.len() {
                out.push(n - 1);
            }
        }
    }
    out.sort();
    out.dedup();
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_items_returns_empty_selection() {
        let out = multi_select(&[], "title");
        assert_eq!(out, Some(Vec::new()));
    }
}
```

**Note on the render/cleanup coupling:** This reuses `arrow_select::cleanup`
which takes a `total_lines` count. Keep the `total_lines` computation in sync
with what you render (title + blank + items + hint = items.len() + 3). If the
screen clears improperly in manual testing, increment the count by one and
re-test.

- [ ] **Step 5: Declare the module and the helper**

In `src/tui/mod.rs`, add `pub mod multi_select;` next to the existing `pub mod arrow_select;`.

Then expose a higher-level helper in `src/operations/delete_batch.rs`:

```rust
fn interactive_select(main_repo: &std::path::Path) -> Result<Vec<String>> {
    let feature_worktrees = git::get_feature_worktrees(Some(main_repo))?;
    if feature_worktrees.is_empty() {
        eprintln!("No feature worktrees to delete.");
        return Ok(Vec::new());
    }
    let labels: Vec<String> = feature_worktrees
        .iter()
        .map(|(branch, path)| format!("{:<30} {}", branch, path.display()))
        .collect();
    let chosen = crate::tui::multi_select::multi_select(&labels, "Select worktrees to delete:");
    match chosen {
        Some(indices) => Ok(indices
            .into_iter()
            .map(|i| feature_worktrees[i].0.clone())
            .collect()),
        None => {
            eprintln!("Cancelled.");
            Ok(Vec::new())
        }
    }
}
```

- [ ] **Step 6: Wire `-i` path in `delete_worktrees`**

Replace the `interactive` branch stub in `delete_worktrees`:

```rust
    let initial_inputs: Vec<String> = if interactive {
        let main_repo = git::get_main_repo_root(None)?;
        let selected = interactive_select(&main_repo)?;
        if selected.is_empty() {
            return Ok(1); // cancelled or nothing selected
        }
        selected
    } else if inputs.is_empty() {
        return legacy_single_current(flags, lookup_mode);
    } else {
        inputs
    };
```

- [ ] **Step 7: Add a unit test that the `-i` path is no longer a hard error**

There is no easy automated test for the TUI itself; gate the real UI behind the TTY check. Add a minimal compile-time check: `cargo build --release` should succeed, and the manual smoke test below must pass. Update `tests/test_cli.rs`:

```rust
#[test]
fn test_delete_interactive_help_mentions_multiselect() {
    cw().args(["delete", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--interactive"));
}
```

- [ ] **Step 8: Run tests**

Run: `cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test`
Expected: all tests pass. The compile-time + help-text check is the automated coverage; TUI interaction itself is validated manually.

- [ ] **Step 9: Manual smoke test**

```bash
cd /tmp/smoke
cargo run -q --manifest-path /Users/dave/Projects/github.com/git-worktree-manager-feat-cw-delete-multiple-worktree/Cargo.toml -- new x --no-term
cargo run -q --manifest-path /Users/dave/Projects/github.com/git-worktree-manager-feat-cw-delete-multiple-worktree/Cargo.toml -- new y --no-term
cargo run -q --manifest-path /Users/dave/Projects/github.com/git-worktree-manager-feat-cw-delete-multiple-worktree/Cargo.toml -- delete -i
```

Expected: TUI opens, arrow keys move the cursor, space toggles marks, Enter submits selection, the normal summary + confirmation path runs. Esc cancels and nothing is deleted.

- [ ] **Step 10: Commit**

```bash
git add src/tui/multi_select.rs src/tui/mod.rs src/operations/delete_batch.rs tests/test_cli.rs
git commit -m "feat(tui): add multi-select widget and wire gw delete -i"
```

---

## Task 7: Integration tests for multi-target delete

**Files:**
- Create: `tests/test_delete_multi.rs`

- [ ] **Step 1: Scaffold the test file using the existing `common::TestRepo` harness**

Create `tests/test_delete_multi.rs`:

```rust
//! Integration tests for multi-target `gw delete`.
//!
//! Uses the `TestRepo` harness from `tests/common/`.

mod common;
use common::TestRepo;

#[test]
fn test_delete_multiple_positional_all_succeed() {
    let repo = TestRepo::new();
    assert!(repo.cw_ok(&["new", "a", "--no-term"]));
    assert!(repo.cw_ok(&["new", "b", "--no-term"]));
    assert!(repo.cw_ok(&["new", "c", "--no-term"]));

    let out = repo.cw(&["delete", "a", "b", "c"]);
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));

    let list = repo.cw_stdout(&["list"]);
    assert!(!list.contains(" a "));
    assert!(!list.contains(" b "));
    assert!(!list.contains(" c "));
}

#[test]
fn test_delete_multiple_mixed_valid_and_missing() {
    let repo = TestRepo::new();
    assert!(repo.cw_ok(&["new", "real", "--no-term"]));

    let out = repo.cw(&["delete", "real", "does-not-exist"]);
    // exit code 2: at least one target was not deleted
    assert_eq!(out.status.code(), Some(2));

    let list = repo.cw_stdout(&["list"]);
    assert!(!list.contains("real"));
}

#[test]
fn test_delete_dry_run_does_not_delete() {
    let repo = TestRepo::new();
    assert!(repo.cw_ok(&["new", "p", "--no-term"]));
    assert!(repo.cw_ok(&["new", "q", "--no-term"]));

    let out = repo.cw(&["delete", "p", "q", "--dry-run"]);
    assert!(out.status.success());

    let list = repo.cw_stdout(&["list"]);
    assert!(list.contains("p"));
    assert!(list.contains("q"));
}

#[test]
fn test_delete_keep_branch_applies_to_all_targets() {
    let repo = TestRepo::new();
    assert!(repo.cw_ok(&["new", "k1", "--no-term"]));
    assert!(repo.cw_ok(&["new", "k2", "--no-term"]));

    let out = repo.cw(&["delete", "k1", "k2", "--keep-branch"]);
    assert!(out.status.success());

    let branches = repo.git_stdout(&["branch", "--list"]);
    assert!(branches.contains("k1"));
    assert!(branches.contains("k2"));
}

#[test]
fn test_delete_interactive_conflicts_with_positional_at_runtime() {
    let repo = TestRepo::new();
    let out = repo.cw(&["delete", "-i", "some-target"]);
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("cannot") || err.contains("conflict"));
}

#[test]
fn test_delete_no_args_still_uses_legacy_path() {
    // Running `gw delete` from *inside* a worktree should still delete the
    // current worktree. This exercises the legacy path.
    let repo = TestRepo::new();
    let wt_path = repo.create_worktree("inside-me");

    let mut cmd = repo.cw_cmd();
    cmd.current_dir(&wt_path);
    let out = cmd.arg("delete").output().unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));

    let list = repo.cw_stdout(&["list"]);
    assert!(!list.contains("inside-me"));
}
```

**Note on confirmation prompts in tests:** `assert_cmd` runs the child with pipes, so stdin is not a TTY. Per Task 4, that means the batch prompt auto-confirms. Tests do not need to send `y\n`.

- [ ] **Step 2: Run the new tests**

Run: `cargo test --test test_delete_multi`
Expected: all six tests pass.

- [ ] **Step 3: Run the entire suite to catch regressions**

Run: `cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test`
Expected: all 460+ existing tests pass plus the six new ones. Total: 466+.

- [ ] **Step 4: Commit**

```bash
git add tests/test_delete_multi.rs
git commit -m "test(delete): integration coverage for multi-target delete"
```

---

## Task 8: Documentation + help-text polish

**Files:**
- Modify: `src/cli.rs` (doc comment on `Delete`)
- Modify: `README.md` (if it documents delete)

- [ ] **Step 1: Inspect `README.md` for delete examples**

```bash
grep -n "gw delete\|cw delete" README.md || echo "no mention"
```

If the README documents `gw delete`, update the example block to show both single and multi-target forms plus `-i` and `--dry-run`. Add one concise example like:

```markdown
    gw delete feat/login feat/signup   # delete multiple worktrees at once
    gw delete -i                       # pick from a multi-select UI
    gw delete feat/x --dry-run         # preview without deleting
```

If the README does not currently mention `gw delete`, skip README changes (do not add a new section unprompted).

- [ ] **Step 2: Polish the `Delete` doc comment**

Update the top-of-variant doc comment in `src/cli.rs` to:

```rust
    /// Delete one or more worktrees.
    ///
    /// With no arguments: deletes the current worktree (must be inside one).
    /// With one or more positional targets: deletes each of them, flags apply
    /// to all targets.
    /// With `-i`: opens a multi-select UI.
    ///
    /// Exits 0 on full success, 1 if the batch confirmation was cancelled,
    /// 2 if any target could not be deleted (not found, busy, or an error).
    Delete { ... }
```

- [ ] **Step 3: Run the full verification suite**

Run: `cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test`
Expected: clean green.

- [ ] **Step 4: Commit**

```bash
git add src/cli.rs README.md 2>/dev/null
git commit -m "docs(delete): document multi-target, -i, and --dry-run"
```

---

## Task 9: Final verification

- [ ] **Step 1: Full test run**

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

All three must pass.

- [ ] **Step 2: Manual end-to-end check**

In a throwaway repo:

```bash
gw new a --no-term && gw new b --no-term && gw new c --no-term
gw delete a b c --dry-run        # summary, exit 0
gw delete a                        # legacy single-target path, unchanged
gw delete b c                      # batch prompt, both deleted
gw delete -i                       # TUI (no candidates left — expect "No feature worktrees to delete.")
gw new d --no-term && gw new e --no-term
gw delete -i                       # select d, skip e; only d deleted
gw delete missing-branch           # exit 2, "not found"
```

Verify exit codes with `echo $?` after each invocation.

- [ ] **Step 3: Review the diff against the spec**

Open the spec and walk through each "Behavior" subsection and "Exit codes" row. Check every one against the running binary and the integration tests. Fix any deviation before handing off.

---

## Out of scope (for this plan)

- `sync`, `merge`, `change-base` multi-target.
- Consolidating `clean` into `delete` (`--merged`, `--older-than` on delete).
- Migrating `clean -i` to the new multi-select TUI (possible follow-up; keep it
  as a line prompt for now — no behavior change).
- Extracting raw-mode helpers into a shared `src/tui/raw_mode.rs`. Copy from
  `arrow_select.rs` for now; factor later when both widgets stabilize.
