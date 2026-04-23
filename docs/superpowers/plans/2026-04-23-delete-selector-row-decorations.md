# `gw delete -i` Row Decorations — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add relative age and a yellow `[busy]` badge to each row of the `gw delete -i` multi-select TUI, matching the row layout promised in the original multi-target delete spec.

**Architecture:** Introduce a pure `format_selector_row` helper in `src/operations/display.rs` (next to the existing `format_age`) that composes the row string with padding for branch / age / busy / path. Rewire `src/operations/delete_batch.rs::interactive_select` to feed the helper `(branch, age, busy, path)` instead of the current `format!("{:<30} {}", …)`. No changes to the `multi_select` widget API or selection mechanics.

**Tech Stack:** Rust, `console` crate (ANSI styling), existing `path_age_days`, `format_age`, `busy::detect_busy`, `arrow_select::{visible_len, truncate}`.

---

## File Structure

**Files touched (all modifications, no new source files):**

- `src/operations/display.rs` — add `pub fn format_selector_row(...)` and its unit tests.
- `src/operations/delete_batch.rs` — in `interactive_select`, compute `age` / `busy` per row and call `format_selector_row` instead of the inline `format!`.

The helper goes in `display.rs` (not `console.rs`) because that's where `format_age` already lives and where future callers (`clean -i`, etc.) will naturally look.

---

### Task 1: Add the `format_selector_row` helper with tests

**Files:**
- Modify: `src/operations/display.rs` (append to end, after `diff_worktrees` and before the `#[cfg(test)] mod tests`)

- [ ] **Step 1: Write the failing tests**

Append these tests inside the existing `#[cfg(test)] mod tests` block in `src/operations/display.rs` (the block starts around line 1068, after `diff_worktrees`). Place them after the existing `test_format_age_*` tests:

```rust
    #[test]
    fn format_selector_row_no_busy() {
        let row = format_selector_row("feat/a", "2d ago", false, "feat-a", 30);
        // branch (30) + space + age (9) + space + busy_pad (7) + path
        assert_eq!(row, "feat/a                         2d ago              feat-a");
    }

    #[test]
    fn format_selector_row_busy_contains_badge() {
        let row = format_selector_row("fix/b", "3w ago", true, "fix-b", 30);
        assert!(row.contains("[busy]"), "expected [busy] in row, got: {:?}", row);
        assert!(row.contains("fix/b"));
        assert!(row.contains("3w ago"));
        assert!(row.contains("fix-b"));
    }

    #[test]
    fn format_selector_row_busy_visible_width_matches_no_busy() {
        // ANSI-colored [busy] must occupy the same visible width as 7 spaces,
        // so columns stay aligned under and not-under the cursor.
        let plain = format_selector_row("x", "1d ago", false, "p", 30);
        let busy = format_selector_row("x", "1d ago", true, "p", 30);
        assert_eq!(
            crate::tui::arrow_select::visible_len(&plain),
            crate::tui::arrow_select::visible_len(&busy),
        );
    }

    #[test]
    fn format_selector_row_empty_age_pads_to_nine() {
        let row = format_selector_row("feat/a", "", false, "feat-a", 30);
        // 30 branch + 1 sep + 9 age + 1 sep + 7 busy_pad + 1 sep + path = fixed prefix
        // Verify the path "feat-a" starts at byte 30 + 1 + 9 + 1 + 7 + 1 = 49.
        assert_eq!(&row[49..], "feat-a");
    }

    #[test]
    fn format_selector_row_long_branch_does_not_truncate() {
        let branch = "feat/extra-long-branch-name-well-past-thirty-chars";
        let row = format_selector_row(branch, "1d ago", false, "p", 30);
        assert!(
            row.starts_with(branch),
            "branch must not be truncated, got: {:?}",
            row
        );
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --lib --package git-worktree-manager operations::display::tests::format_selector_row`

Expected: FAIL with a compile error ("cannot find function `format_selector_row` in this scope"). That's the expected "failing test" state for a function that doesn't exist yet.

- [ ] **Step 3: Implement `format_selector_row`**

Add this function to `src/operations/display.rs`. Place it directly after the existing `format_age` function (around line 131). Do not change `format_age` itself.

```rust
/// Compose a single row for the `gw delete -i` multi-select TUI.
///
/// Columns, left to right, separated by one space:
///   branch (padded to `branch_col`) | age (padded to 9) | busy (7, colored) | path
///
/// The busy column carries an ANSI-colored `[busy]` token when `busy` is true,
/// or 7 spaces when false. `arrow_select::visible_len` is ANSI-aware, so the
/// colored and plain variants have identical visible width.
///
/// The path column is appended verbatim. The caller is expected to run the
/// returned string through `arrow_select::truncate` to cap line width; that
/// truncation clips the trailing path column, which is the correct behavior
/// per the row-decorations spec (badges and age must survive, path may shrink).
pub fn format_selector_row(
    branch: &str,
    age: &str,
    busy: bool,
    path: &str,
    branch_col: usize,
) -> String {
    const AGE_COL: usize = 9;
    const BUSY_COL: usize = 7;
    let busy_cell: String = if busy {
        // "[busy]" is 6 visible chars; pad to BUSY_COL with one trailing space.
        format!("{} ", style("[busy]").yellow())
    } else {
        " ".repeat(BUSY_COL)
    };
    format!(
        "{branch:<branch_col$} {age:<AGE_COL$} {busy_cell}{path}",
        branch = branch,
        age = age,
        busy_cell = busy_cell,
        path = path,
        branch_col = branch_col,
        AGE_COL = AGE_COL,
    )
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --lib --package git-worktree-manager operations::display::tests::format_selector_row`

Expected: all 5 new tests PASS. If `format_selector_row_no_busy` fails on the exact string, recount spaces: 30 branch col for `"feat/a"` = 6 chars + 24 spaces; then 1 space sep; then `"2d ago"` = 6 chars + 3 spaces to reach AGE_COL=9; then 1 space sep; then 7 spaces for busy_cell (no trailing sep because `busy_cell` already ends in a space when busy, and when not busy it's 7 spaces with no separator); then `"feat-a"`.

Note: the non-busy `busy_cell` is 7 spaces with no separator after it, so the exact expected string is:

```
"feat/a                         2d ago              feat-a"
 \_________ 30 __________/ \______ 9 ____/ \_ 7 __/\_path_/
```

That's 30 + 1 + 9 + 1 + 7 + `"feat-a"` = 48 + 6 = 54 bytes. If the literal in the test doesn't match, adjust by counting spaces in the expected string to exactly `(30 - 6) = 24` after `feat/a`, then one space, then `2d ago`, then `(9 - 6) = 3` spaces, then one space, then 7 spaces, then `feat-a`.

- [ ] **Step 5: Commit**

```bash
git add src/operations/display.rs
git commit -m "$(cat <<'EOF'
feat(display): add format_selector_row helper for TUI row composition
EOF
)"
```

---

### Task 2: Wire the helper into `delete -i` selector

**Files:**
- Modify: `src/operations/delete_batch.rs:38-42` (the `labels` construction inside `interactive_select`)

- [ ] **Step 1: Read the current `interactive_select` to find the exact lines**

Run: `grep -n "let labels" src/operations/delete_batch.rs`

Expected: `38:    let labels: Vec<String> = feature_worktrees`

The current block is:

```rust
    let labels: Vec<String> = feature_worktrees
        .iter()
        .map(|(branch, path)| format!("{:<30} {}", branch, path.display()))
        .collect();
```

- [ ] **Step 2: Replace the `labels` block with an age + busy enriched version**

Edit `src/operations/delete_batch.rs` — change lines 38-42 from the block above to:

```rust
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
```

The imports at the top of the file already include `use crate::operations::busy::{self, BusyInfo};`, so `busy::detect_busy` resolves without a new `use`. The `path_age_days` and `format_age` calls are fully qualified to avoid adding new `use` lines just for one call site.

- [ ] **Step 3: Verify it compiles**

Run: `cargo build --package git-worktree-manager`

Expected: no errors. Zero warnings is the repo policy.

If the compiler complains about `path_age_days` visibility, confirm it is `pub` in `src/constants.rs` (it already is at line 264). If it complains about `format_age` visibility, confirm `pub fn format_age` in `src/operations/display.rs` at line 114 (it already is).

- [ ] **Step 4: Run the full test suite**

Run: `cargo test --package git-worktree-manager`

Expected: all tests pass. Pay attention to any `delete_batch` or `display` tests — the wiring change should not break them.

- [ ] **Step 5: Commit**

```bash
git add src/operations/delete_batch.rs
git commit -m "$(cat <<'EOF'
feat(tui): show relative age and busy badge in delete -i selector

Each row in the `gw delete -i` multi-select now shows branch, relative
age (reusing format_age), a yellow [busy] badge when busy::detect_busy
reports processes holding the worktree, and the path. The multi_select
widget API is unchanged; only the label composition in
delete_batch::interactive_select changes.
EOF
)"
```

---

### Task 3: Verify formatting, lint, and binary size

**Files:** none modified; verification only.

- [ ] **Step 1: Verify formatting**

Run: `cargo fmt --check`

Expected: no output, exit 0.

If fmt reports drift, run `cargo fmt` and amend… no, create a new commit:

```bash
cargo fmt
git add -u
git commit -m "style: cargo fmt"
```

- [ ] **Step 2: Verify clippy with full lint coverage**

Run: `cargo clippy --all-targets --all-features -- -D warnings`

Expected: 0 warnings, exit 0. This matches the pre-release checklist in `CLAUDE.md`.

If clippy complains inside the new code, fix it in place and commit with:

```bash
git add -u
git commit -m "chore: fix clippy warnings"
```

- [ ] **Step 3: Full test run (all targets)**

Run: `cargo test --all-targets`

Expected: all tests pass.

- [ ] **Step 4: Release build sanity check**

Run: `cargo build --release && ls -lh target/release/gw`

Expected: build succeeds, binary size near the ~1.9MB baseline from `CLAUDE.md`. A small increase (few KB) is acceptable; tens of KB would warrant explanation but is not a blocker.

- [ ] **Step 5: Manual smoke test (optional but recommended)**

In a scratch repo with 2+ feature worktrees, run `gw delete -i` and confirm the row layout shows `branch | age | [busy]? | path`. If no busy worktree is available, open a shell `cd` into one of the worktrees in another terminal and re-run to see the yellow `[busy]` badge.

This step is user-facing and cannot be automated. Note it in the PR description as "manual smoke tested" or skip if no scratch repo is handy.

---

## Self-Review

**Spec coverage:**
- Row layout (branch, age, busy, path): Task 1 defines the helper, Task 2 wires it ✓
- Age via `format_age` / `path_age_days`: Task 2, Step 2 ✓
- Busy badge yellow, via `busy::detect_busy`: Task 1 (yellow styling), Task 2 (call) ✓
- Truncation targets path (badges protected): achieved by column order; Task 1 helper puts path last, so `arrow_select::truncate` in `multi_select::render` clips the tail ✓
- Fallback path (non-TTY): no change needed — `multi_select_fallback` consumes the same `Vec<String>` and will pick up the new labels automatically. Noted in spec, no task required ✓
- Unit test for `format_selector_row`: Task 1, Step 1 ✓
- ANSI stripping for visible-width assertion: Task 1 test `format_selector_row_busy_visible_width_matches_no_busy` uses `arrow_select::visible_len` ✓
- Behavior preservation (exit codes, selection mechanics): no changes to those code paths; only label content ✓
- Conventional commits, no `!` / `BREAKING CHANGE:`: Task 2 commit uses `feat(tui):` ✓
- Zero clippy warnings: Task 3, Step 2 ✓

**Placeholder scan:** no TBDs, every code step has complete code, every test has the expected output described.

**Type consistency:** `format_selector_row(branch: &str, age: &str, busy: bool, path: &str, branch_col: usize) -> String` is used consistently in Task 1 (definition), Task 1 tests (all five call sites), and Task 2, Step 2 (wiring call site). Argument order matches.

---

## Execution handoff

Plan complete and saved to `docs/superpowers/plans/2026-04-23-delete-selector-row-decorations.md`. Two execution options:

**1. Subagent-Driven (recommended)** — dispatch a fresh subagent per task, review between tasks, fast iteration.

**2. Inline Execution** — execute tasks in this session using executing-plans, batch execution with checkpoints.

Which approach?
