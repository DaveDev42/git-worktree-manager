# Delete Multiple Worktrees — Design

Date: 2026-04-23
Branch: `feat/cw-delete-multiple-worktree`

## Goal

Allow `gw delete` to remove more than one worktree in a single invocation, via
either positional arguments or an interactive multi-select UI, while preserving
all existing single-target behavior and flags.

## Motivation

`gw delete` today accepts one positional target at most. Users who want to clean
up several feature branches must invoke the command repeatedly. `gw clean`
exists for bulk cleanup, but it is filter-driven (`--merged`,
`--older-than`) — it does not cover the "delete these three specific worktrees"
case.

## Non-goals

- Multi-target support for `sync`, `merge`, `change-base`, `pr`, `resume`,
  `stash apply`, `backup create/restore`. Those commands keep their current
  shape. They are candidates for a follow-up, not this work.
- Consolidating `gw clean` into `gw delete` (e.g. `delete --merged`,
  `delete --older-than`). Observe real usage of multi-target `delete` first,
  then decide in a separate RFC.
- Filter combinations inside `delete -i` (`-i --merged`, `-i --older-than`).
  Those remain the responsibility of `clean -i`.

## CLI surface

`Commands::Delete` in `src/cli.rs` changes as follows:

```rust
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

    // Unchanged:
    // -k/--keep-branch, -r/--delete-remote,
    // -f/--force, --no-force,
    // -w/--worktree, -b/--branch
}
```

Parsing:

- Zero positional + no `-i` → preserves the legacy "delete current worktree"
  behavior.
- One or more positional → multi-target mode; all positional flags apply to
  every target.
- `-i` alone → launches the TUI selector.
- `-i` together with positional → clap rejects at parse time.

## Behavior

### Target selection

```
entry (delete cmd)
  │
  ├─ interactive && targets.empty()  → TUI select → selected = [...]
  ├─ targets.len() >= 1              → selected = targets
  └─ else                             → selected = [current worktree]   (legacy)
```

### Resolve, plan, confirm, execute

```
resolve_all(selected, lookup_mode)
  │     per target → Resolved { path, branch } | Unresolved { input, reason }
  ▼
plan(resolved)            # busy detection per entry (reuses busy::detect_busy)
  │
  ▼
print_summary             # "N to delete (M busy, K not found):" + list
  │
  ├─ dry_run              → exit 0
  │
  ├─ len > 1 && TTY       → "Proceed? (y/N)" once for the whole batch
  │
  ▼
execute_each              # best-effort, sequential
  │     per entry: pre_delete hook → git remove → branch + metadata →
  │                 remote branch (if requested) → post_delete hook
  │     each failure is caught into results
  ▼
print_results             # "1 deleted, 2 skipped" + per-item outcome lines
  ▼
exit code
```

Key points:

- **Resolve-all-first.** All inputs are resolved before any deletion begins.
  Anything that does not resolve is shown in the summary and skipped at
  execute time.
- **Single batch confirmation.** When two or more targets are planned and
  stdin/stderr are both TTYs, we prompt once for the whole batch. Legacy
  single-target invocations do *not* get this new prompt; busy-gate prompting
  for a single target continues to behave exactly as today.
- **Per-target independence.** A failure on one target never aborts the batch.
  Hooks, branch deletion, and remote branch deletion all run per target.
- **Flags apply to every target.** `-k`, `-r`, `-f`, `--no-force`, `-w`, `-b`
  cover the whole batch. This mirrors the Unix convention of `rm -f a b c`.

### Failure handling and exit codes

| Situation | Action | Exit contribution |
|---|---|---|
| Resolve miss (not found) | listed in summary, skipped | non-zero |
| Busy without `--force` | listed in summary, skipped | non-zero |
| `git worktree remove` fails | recorded, batch continues | non-zero |
| Branch delete fails after worktree removed | warning only | zero ("partial success") |
| Remote branch push fails | warning only (legacy behavior) | zero |
| Pre-delete hook fails for one target | that target aborts, batch continues | non-zero |
| User answers N at batch prompt | nothing deleted | 1 (cancelled) |

Exit codes:

- `0` — every requested target was deleted, or `--dry-run`.
- `1` — user cancelled at the confirmation prompt.
- `2` — the batch completed but at least one target was not deleted
  (unresolved, busy-skipped, remove failure, hook failure).

### Output format

Dry-run example:

```
Would delete 3 worktrees:
  feat/a       /path/to/feat-a
  feat/b       /path/to/feat-b      (busy: 2 process(es))
  unknown/x    — not found
Total: 3 planned, 1 not found, 1 busy
(dry-run; nothing deleted)
```

Real execution example:

```
Deleting 3 worktrees (1 busy, will skip without --force):
  feat/a
  feat/b      (busy: 2 process(es))  [skip]
  unknown/x   [not found] [skip]
Proceed? (y/N): y

• Removing feat/a ... ✓
• Skipped feat/b (busy)
• Skipped unknown/x (not found)

Summary: 1 deleted, 2 skipped
```

Single-target output is unchanged.

### Interactive (`-i`) UI

- Opens a TUI checkbox list of all worktrees in the repo, excluding the main
  worktree (we never delete it).
- Each row shows branch/worktree name, path, relative age, and a busy badge
  when applicable.
- Space toggles selection; Enter confirms; Esc/q cancels.
- Prefer reusing the selection widget that currently powers `clean -i`. If it
  is not cleanly separable, extract a small shared multi-select component under
  `src/tui/` so both commands share it.
- After Enter, the selected set flows through the same resolve → plan → summary
  → confirm → execute pipeline as positional input. This keeps one code path
  for the dangerous part (execution).
- `-i --dry-run` prints the dry-run summary for the selection and exits.
- `-i` with `-k`, `-r`, `-f`, etc. applies those flags to every selected item.
- Confirming with nothing selected prints `Nothing selected` and exits `0`.
- Esc / q prints `Cancelled` and exits `1`.

## Implementation

### Files and responsibilities

- `src/cli.rs` — update `Commands::Delete` as in the CLI surface section.
- `src/entrypoint.rs` — route `Commands::Delete` to a new orchestrator
  `worktree::delete_worktrees(...)` that returns the final exit code
  (or propagates `Result` and maps internally).
- `src/operations/worktree.rs`
  - Split today's `delete_worktree()` into:
    - `delete_one(resolved, flags, hook_ctx_builder) -> DeletionOutcome` —
      pure per-target logic. No summary printing, no batch prompt, no busy
      prompting. It assumes the orchestrator has already decided to proceed.
    - `delete_worktrees(inputs, flags) -> Result<i32>` — orchestrator. Handles
      target selection (targets / interactive / current), resolve-all, plan,
      summary printing, dry-run short-circuit, batch confirmation, sequential
      execution, result aggregation, and exit code computation.
  - Preserve the legacy single-target prompt/busy path: when the orchestrator
    sees exactly one input and it came from the legacy "no args" branch, it
    delegates to a thin single-target path that keeps the current
    `Delete anyway? (y/N)` semantics. This guarantees backward compatibility
    for the common `gw delete` workflow inside a worktree.
- `src/operations/busy.rs` — no change. Reuse `detect_busy()`.
- `src/tui/` — expose `select_worktrees_interactive(repo) -> Result<Vec<Selected>>`.
  Refactor only as much as needed to share the widget with `clean -i`.

### Data types (sketch)

```rust
enum TargetInput {
    Legacy,              // from "gw delete" with no args, no -i
    Explicit(String),    // from positional
    Interactive(Selected),
}

struct Resolved {
    input: String,
    path: PathBuf,
    branch: Option<String>,
}

enum PlanEntry {
    Ready(Resolved),
    Busy(Resolved, Vec<BusyEntry>),
    Unresolved { input: String, reason: String },
}

enum DeletionOutcome {
    Deleted { input: String },
    Skipped { input: String, reason: String },
    Failed  { input: String, error: CwError },
}
```

### Backward compatibility

- `gw delete` (no args) → deletes current worktree, same output, same busy
  prompt. No new batch prompt.
- `gw delete feat/a` → identical to today.
- Adding a second positional (`gw delete feat/a feat/b`) is what opts the user
  into the new batch pipeline.
- All existing CLI flags keep their names, short forms, and semantics.

## Testing

- **Existing single-target tests** — must all pass unchanged.
- **New integration tests** in `tests/`:
  - multiple positional, all valid → all deleted, exit 0.
  - mix of valid and non-existent targets → valid ones deleted, summary lists
    the miss, exit 2.
  - busy target without `--force` → skipped with reason, other targets
    deleted, exit 2.
  - busy target with `--force` → deleted like the rest.
  - `--dry-run` with multiple targets → nothing deleted, summary printed,
    exit 0.
  - `--delete-remote` with multiple targets → remote delete attempted for
    each.
  - batch confirmation prompt answered `n` → nothing deleted, exit 1.
  - `-i` and positional together → clap parse error.
  - `gw delete` with no args inside a worktree → legacy behavior preserved.
- **TUI selection test** — mirror the existing `clean -i` test harness
  (may be `#[ignore]` by default).

## Open questions

None at spec time. Any ambiguity discovered during implementation is resolved
in favor of preserving legacy single-target behavior.

## Out of scope (follow-ups)

- Multi-target `sync a b c` — `--all` covers most cases, but "these three
  only" is a real gap. Separate design.
- `merge`, `change-base` multi-target — needs its own conflict/rollback story.
- `clean` vs `delete` consolidation — revisit after this ships and real usage
  data is available.
