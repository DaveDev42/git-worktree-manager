# `gw delete -i` Row Decorations — Design

Date: 2026-04-23
Branch: `feat-tui-row-decorations`

## Goal

Enrich each row in the `gw delete -i` multi-select TUI with two signals already
called out in the original multi-target delete spec
(`2026-04-23-delete-multiple-worktrees-design.md`, lines 172-173) but not yet
implemented:

- **Relative age** (e.g. `2d ago`, `3w ago`, `1mo ago`) — reuses `format_age`
  from `src/operations/display.rs` and `path_age_days` from `src/constants.rs`.
- **Busy badge** — a yellow `[busy]` marker when
  `busy::detect_busy(path)` reports at least one process holding the worktree.

The current selector label is only `branch + path` (see
`src/operations/delete_batch.rs::interactive_select`), which does not match the
spec and hides information the user needs to decide what to delete.

## Non-goals

- Migrating `clean -i` to the multi-select widget — out of scope (already noted
  as a follow-up in the parent spec).
- Refactoring `gw list` column layout to share more code — a targeted helper
  is enough; `gw list` stays on its ratatui table path.
- Sorting or filtering the selector list — order follows whatever
  `git::get_feature_worktrees` returns today.

## Row layout

Left to right, with one space between columns:

```
[cursor marker] [branch, ≤30 cols] [age, 9 cols] [busy badge, 7 cols] [path, remaining]
```

Example output (80-col terminal, cursor on the second row):

```
Select worktrees to delete:

    [ ] feat/alpha                  2d ago            feat-alpha
  > [x] fix/logging                 3w ago   [busy]   fix-logging
    [ ] chore/bump-deps             just now          chore-bump-deps

  (Space: toggle, Enter: confirm, Esc/q: cancel)
```

### Column order rationale

Spec line 173 lists the fields as `name, path, relative age, busy badge`.
For this work we reorder to `name, age, busy, path` because:

1. **Truncation targets path.** The prompt requires badges and age to never be
   truncated — only the path column is allowed to shrink. Putting `path` last
   means the existing `arrow_select::truncate(line, width)` call in
   `multi_select::render` clips the path column by construction, with zero
   extra logic.
2. **Age and busy are narrow and high-signal.** They live next to the branch
   name so the eye can scan a short, fixed-width strip of metadata before
   hitting the noisier path column.

### Column widths

- **Branch:** 30 cols (matches current layout). Branches longer than 30 are
  left unpadded — they push the later columns right, same as today.
- **Age:** 9 cols. Covers `just now` (8), `23h ago` (7), `12mo ago` (8),
  `999y ago` (9). Missing age (path gone) renders as 9 spaces.
- **Busy:** 7 cols. `[busy]` is 6 visible chars; 1 trailing space keeps the
  path column separated. When not busy, 7 spaces.
- **Path:** remaining width, truncated by `arrow_select::truncate`.

### Styling

- Branch and path: plain text.
- Age: plain text (caller may add dim color later; not in this change).
- Busy badge: `console::style("[busy]").yellow()` so it stands out even under
  the cursor row's inverse-video highlight.

`arrow_select::truncate` and `visible_len` are ANSI-aware, so embedded color
escapes in the busy column do not confuse width calculations.

## Implementation

### New helper

Add to `src/operations/display.rs` (next to `format_age`):

```rust
/// Compose a single row for the `gw delete -i` multi-select TUI.
///
/// Columns: branch (padded), age (padded), busy badge or padding, path.
/// The busy badge carries ANSI color; all other columns are plain text.
///
/// `branch_col` is the width reserved for the branch column. Callers pass
/// 30 today; exposed as a parameter so future callers (e.g. `clean -i`) can
/// tune it without forking the helper.
pub fn format_selector_row(
    branch: &str,
    age: &str,
    busy: bool,
    path: &str,
    branch_col: usize,
) -> String;
```

Behavior:

- Pads `branch` to `branch_col` using `{:<width$}` — if `branch` is longer,
  it is printed as-is (no truncation here; `arrow_select::truncate` handles
  the final line cap).
- Pads `age` to 9 visible cols.
- Writes either a 7-col yellow `[busy]` (with trailing space) or 7 spaces.
- Appends `path` verbatim (no truncation; caller-level truncate handles it).

Pure composition — no I/O, no side effects.

### Call-site wiring

`src/operations/delete_batch.rs::interactive_select`:

- For each `(branch, path)` in `feature_worktrees`:
  - `age = path_age_days(path).map(format_age).unwrap_or_default()`
  - `busy = !busy::detect_busy(path).is_empty()`
  - `label = format_selector_row(branch, &age, busy, &path.display().to_string(), 30)`
- Feed the resulting `Vec<String>` to `multi_select::multi_select` as today.

Fallback path (`multi_select_fallback` in `src/tui/multi_select.rs`): the same
`Vec<String>` of labels is printed via `eprintln!`, so it automatically picks
up the new content. No fallback-specific change needed; ANSI in non-TTY
fallback is degraded gracefully by terminals that ignore escapes, which is the
same behavior the `busy` color already has in `print_summary`.

### Busy-scan cost

`busy::detect_busy` runs `lsof`/ps under the hood (~1.5s on macOS for a full
scan). The selector lists only feature worktrees (typically <20), and is
already an interactive blocking prompt, so running `detect_busy` per row is
acceptable. No caching.

## Testing

Unit tests in `src/operations/display.rs`:

- `format_selector_row` with no busy → branch padded, age padded, 7 spaces,
  path appended. Assert on the raw string (no ANSI involved).
- `format_selector_row` with busy=true → result contains `[busy]` and starts
  with the expected branch padding. Strip ANSI via a local helper (or reuse
  `arrow_select::visible_len`) to assert total visible width.
- `format_selector_row` with empty age → 9 spaces in the age column.
- `format_selector_row` with a branch longer than `branch_col` → branch is
  not truncated, and the age column still starts immediately after.

No integration test for the TUI itself — raw-mode interaction is not
automatable. The composition helper is the testable unit.

## Backward compatibility

- The `multi_select` widget's `&[String]` API is unchanged.
- Exit codes and selection mechanics in `delete_batch.rs` are unchanged —
  only the label strings change.
- Non-TTY fallback still works; it prints the same (richer) labels.

## Out of scope / follow-ups

- Colorizing the age column (dim) once `clean -i` migrates and shares the
  helper.
- Per-column truncation (branch or path) with explicit ellipsis — revisit if
  users report unreadable rows on very narrow terminals.
- Sorting the selector by age or status.
