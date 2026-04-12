# `gw list` Performance & Progressive Rendering Design

**Status:** Draft
**Date:** 2026-04-13
**Related:** Complaints of slow `gw list` in repositories with many worktrees (e.g. `magicmoment`)

## Problem

`gw list` executes the following work sequentially for each worktree:

1. `busy::detect_busy` — lockfile and process scan
2. `gh pr view <branch>` — **network round-trip**, typically hundreds of ms to several seconds
3. `git branch --merged` — local fallback when `gh` is absent
4. `git status --porcelain` — working tree scan

In a repository with N worktrees the worst case is `N × (gh_latency + local_io)`. On `magicmoment`-scale repos this produces multi-second stalls with no feedback to the user.

## Goals

- Make `gw list` feel instantaneous in common cases.
- Show the worktree list to the user immediately, update statuses progressively.
- Handle terminal resize, Ctrl-C, and non-TTY invocations gracefully.
- Establish a reusable TUI foundation for future commands (`gw watch`, interactive `gw tree`, etc.).

## Non-Goals

- Rewriting other commands to use the TUI layer in this change. Only `gw list` migrates. Other commands continue to use `src/console.rs` style helpers.
- Replacing `gh` with a native GitHub API client.
- Persisting worktree status across `gw` invocations beyond PR state.

## Approach

Four changes applied together:

**A. Parallel status computation** using `rayon` at the default pool size (CPU cores).

**B. Progressive rendering** using `ratatui` with an Inline Viewport. The worktree table is drawn immediately with placeholder statuses, then updated in place as each worktree's status is computed.

**C. Batched PR status** via a single `gh pr list --state all --json headRefName,state --limit 500` call instead of one `gh pr view` per worktree.

**D. On-disk PR cache** at `~/.cache/gw/pr-status-<repo-hash>.json` with a 60-second TTL and a `--no-cache` flag for explicit invalidation.

## Architecture

### New modules

- `src/operations/pr_cache.rs` — batched `gh pr list` invocation, XDG cache read/write, TTL.
- `src/tui/mod.rs` — `ratatui::Terminal` setup/teardown helpers, panic hook installation.
- `src/tui/style.rs` — style palette shared between ratatui and `console` crate (color constants in one place, two renderers).
- `src/tui/list_view.rs` — `gw list` Inline Viewport application.

### Modified files

- `src/operations/display.rs` — `get_worktree_status` signature changes to accept `&PrCache`; `list_worktrees` dispatches to TUI path when stdout is a TTY, static path otherwise.
- `src/cli.rs` — add `--no-cache` flag to the `list` subcommand.
- `src/main.rs` — install global panic hook that restores terminal state.
- `Cargo.toml` — add `ratatui`, `crossterm`, `rayon`, `sha2`.

### Existing modules untouched

`src/console.rs`, `src/operations/busy.rs`, and the remaining `src/operations/*.rs` files remain unchanged. The TUI layer is strictly additive.

## Component Design

### PR Cache (`src/operations/pr_cache.rs`)

**Location:** `~/.cache/gw/pr-status-<repo-hash>.json` via the `dirs` crate (XDG standard on Linux, `~/Library/Caches` on macOS, `%LOCALAPPDATA%` on Windows).

**Repo hash:** first 16 hex chars of SHA-256 over the canonicalized absolute path of the repo root. Keeps caches for different repos isolated.

**TTL:** 60 seconds.

**Schema:**

```json
{
  "fetched_at": 1712345678,
  "repo": "/Users/dave/Projects/github.com/magicmoment",
  "prs": { "feat/foo": "OPEN", "fix/bar": "MERGED", "chore/baz": "CLOSED" }
}
```

**Public API:**

```rust
pub struct PrCache {
    map: std::collections::HashMap<String, String>,
}

impl PrCache {
    /// Load from disk if fresh, else fetch via `gh pr list` and persist.
    /// Returns an empty cache on any failure (gh missing, network error, disk error).
    pub fn load_or_fetch(repo: &Path, no_cache: bool) -> Self;

    /// Return PR state for a branch, or None if the branch has no known PR.
    pub fn state(&self, branch: &str) -> Option<&str>;
}
```

**Behavior:**

- `no_cache == true`: skip the disk read, always fetch.
- Cache hit and `now - fetched_at < 60`: return parsed map.
- Cache miss / expired / corrupt / `no_cache`: run `gh pr list --state all --json headRefName,state --limit 500`, parse, write to disk, return.
- `gh` missing or returns non-zero: return an empty `PrCache`. The caller's fallback (`git branch --merged`) handles merge detection.
- Disk write failures are silently ignored (in-memory result still usable).
- Cache directory is created lazily; failure to create it means the result is returned without persistence.

**Test hooks:**

- `GW_TEST_GH_JSON` env var: if set, parsed as the `gh` output instead of spawning `gh`.
- `GW_TEST_GH_FAIL=1`: simulate `gh` returning non-zero.

### Status Computation

**Signature change** in `src/operations/display.rs`:

```rust
pub fn get_worktree_status(
    path: &Path,
    repo: &Path,
    branch: Option<&str>,
    pr_cache: &PrCache,
) -> String
```

The PR-state check inside the function uses `pr_cache.state(branch)` instead of calling `git::get_pr_state`. If the cache has no entry for the branch, the existing `git::is_branch_merged` fallback runs unchanged.

All other call sites (`doctor`, `stats`, etc.) must be updated to pass a `PrCache`. Call sites that run once per command can construct a cache via `PrCache::load_or_fetch` at the top of the command.

### Parallel Computation

`list_worktrees` prepares a vector of row inputs (path, branch, worktree_id, age, rel_path) serially — this is fast local work. Then it hands the vector to the renderer.

In the static (non-TTY) path the renderer uses `rayon::prelude::ParallelIterator` directly:

```rust
let rows: Vec<WorktreeRow> = inputs.par_iter()
    .map(|input| compute_row(input, &repo, &pr_cache))
    .collect();
```

In the progressive (TTY) path the renderer spawns a rayon task that sends `(row_index, status)` tuples over `std::sync::mpsc::channel`. The main thread owns the `ratatui::Terminal` and consumes the channel (see below).

Pool size is rayon's default (CPU cores). With PR lookups batched into a single `gh` call, the remaining per-worktree work is local git/fs I/O — a larger pool would only invite git internal lock contention.

Thread safety notes:

- `std::env::current_dir()` is a read-only syscall, safe across threads.
- `git::git_command` spawns a child process per call; no shared mutable state.
- `PrCache` is passed by `&` reference (or `Arc` in the TUI path). `HashMap` read access is thread-safe.

### Progressive Rendering (`src/tui/list_view.rs`)

**Library:** `ratatui` with `CrosstermBackend`, using `Viewport::Inline(height)`. The viewport occupies N lines below the current cursor position, does not enter the alternate screen, and leaves its final frame in the scrollback on exit — preserving the stream-output feel of a CLI.

**Flow:**

1. Determine viewport height from input count (`inputs.len() + 4` for header, borders, and trailing blank line).
2. Construct `Terminal::with_options` in Inline mode.
3. Spawn a rayon task that computes statuses in parallel and sends `(usize, String)` over `mpsc`.
4. Render skeleton frame: all rows present, status cell shows `"…"` in dim style.
5. Main loop: `rx.recv_timeout(50ms)`. On each message, update `app.rows[i].status` and re-`draw`. On `Disconnected` or when all rows are filled, break.
6. `drop(terminal)` exits the viewport; ratatui leaves the final frame in scrollback.
7. `println!` the summary footer after the viewport exits — it naturally lands below the table.

**App state:**

```rust
struct ListApp {
    inputs: Arc<Vec<RowInput>>,
    rows: Vec<WorktreeRow>,  // parallel-indexed with inputs
    complete_count: usize,
}
```

`ListApp::render(frame)` builds a `Table` widget each frame from `rows`. ratatui's diffing ensures only changed cells are redrawn in the terminal.

**Styling:** `src/tui/style.rs` exposes a function that maps status strings ("clean", "modified", "busy", "active", "pr-open", "merged", "stale") to `ratatui::Style`. The color constants mirror `src/console.rs::status_style` so both paths render with visually identical colors.

**TTY detection:** `crossterm::tty::IsTty` on `std::io::stdout()`. If false, `list_worktrees` bypasses `tui::list_view` entirely and uses the static renderer.

### Edge Cases Handled by ratatui

- **Terminal resize:** crossterm emits resize events; Inline Viewport re-computes layout automatically on the next draw.
- **Panic:** `src/main.rs` installs `std::panic::set_hook` that calls `ratatui::restore()` (disables raw mode, shows the cursor) before delegating to the default hook.
- **Ctrl-C:** Inline Viewport does not enable raw mode by default; SIGINT terminates the process with the terminal in a usable state. No explicit signal handler needed. If we later enable raw mode for interactive variants, install a `Drop` wrapper on `Terminal` that restores on drop.
- **Unicode width / wrapping:** `ratatui::widgets::Table` handles cell width and truncation.
- **Small terminals:** below a threshold width the TUI path falls back to the static compact layout (same threshold as today: `MIN_TABLE_WIDTH = 100`).

## Error Handling

| Failure | Behavior |
|---|---|
| `gh` missing / fails | Empty `PrCache`; `git branch --merged` fallback runs per-worktree. |
| Cache file corrupt | Silently re-fetch. |
| Cache write fails | Use in-memory result, no persistence. |
| Cache dir creation fails | Use in-memory result, no persistence. |
| rayon worker panic | Propagated to main thread, surfaced as `CwError`. `get_worktree_status` does not panic today; it encodes all errors as status strings. |
| mpsc send error | `let _ = tx.send(...)`. Receiver drop means we're shutting down. |
| ratatui draw error | Returned as `CwError::Io`. Terminal is restored via Drop. |
| Non-TTY stdout | Skip TUI path entirely; use static renderer. |

## CLI Changes

```rust
// src/cli.rs, List variant
List {
    #[arg(long, help = "Bypass PR status cache and refresh from gh")]
    no_cache: bool,
},
```

No other user-visible flags change. Default behavior uses cache with 60s TTL.

## Dependencies

```toml
ratatui = "0.28"
crossterm = "0.28"     # ratatui's default backend; explicit for IsTty trait
rayon = "1.10"
sha2 = "0.10"
# dirs is likely already present; add if not
```

Expected binary size: ~1.9MB → ~2.3MB.

## Testing

### `pr_cache.rs` unit tests

- `test_cache_miss_calls_gh` — first call invokes `gh` (via `GW_TEST_GH_JSON`).
- `test_cache_hit_within_ttl` — second call within 60s reads disk only.
- `test_cache_expired_refetches` — manipulate `fetched_at` to past, verify refetch.
- `test_no_cache_flag_bypasses` — `no_cache=true` skips disk.
- `test_corrupt_cache_falls_back` — invalid JSON triggers silent refetch.
- `test_repo_hash_isolates` — different repo paths produce different cache files.
- `test_gh_failure_returns_empty_cache` — `GW_TEST_GH_FAIL=1` yields empty `PrCache`.

### `display.rs` updates

- Existing `get_worktree_status` tests updated to pass an empty `PrCache`.
- `test_status_uses_pr_cache` — cache containing `MERGED` returns "merged" without calling `gh` or `git branch --merged`.

### `tui/list_view.rs` tests

- Use `ratatui::backend::TestBackend` to snapshot skeleton frame (all `"…"` statuses) and a fully-populated frame.
- Progressive update path is exercised in a unit test by sending pre-computed `(index, status)` tuples into the renderer's input channel and asserting buffer contents after each draw.

### Manual verification

Run `time gw list` in `magicmoment` before and after. Include before/after numbers in the PR description. Visually confirm progressive update (skeleton → fill) and that the final frame remains in scrollback after the command exits.

## Open Questions

None at this time. All design decisions have been made collaboratively with the user:

- PR cache location: `~/.cache/gw/pr-status-<repo-hash>.json` (XDG).
- TTL: 60s with `--no-cache` override.
- Parallelism: rayon default pool size.
- Non-TTY: static rendering (no progressive updates).
- Progressive UX: skeleton first, in-place status updates.
- TUI library: `ratatui` with Inline Viewport.

## Rollout

Single PR. No feature flag. Changes are internal to `gw list` rendering and additive at the module level; the CLI surface gains only `--no-cache`.
