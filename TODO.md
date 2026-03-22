# TODO - git-worktree-manager (gw)

This document tracks planned features, enhancements, and known issues for the Rust rewrite.

## High Priority

- [x] **A-1: Magic number constants** — 8 occurrences → `constants.rs`
  - `SECS_PER_DAY` / `SECS_PER_DAY_F64`, `MIN_GIT_VERSION*`, `AI_TOOL_TIMEOUT_SECS`, `AI_TOOL_POLL_MS`

- [x] **A-2: `home_dir_or_fallback()` helper** — 6 duplicated call sites → `constants.rs`

- [x] **A-3: `path_age_days()` helper** — 4 duplicated call sites → `constants.rs`

- [x] **A-5: `version_meets_minimum()` helper** + unit tests → `constants.rs`

- [x] **Shell completion auto-prompt** — Python `prompt_completion_setup()` parity
  - `prompt_shell_completion_setup()` in `config.rs`, called on startup
  - One-time hint to run `gw shell-setup`; marks `shell_completion.prompted`/`.installed`

- [x] **Shell script syntax tests** — validate generated scripts
  - `bash -n` / `fish --no-execute` in `shell_functions::tests`
  - Gracefully skip if shell binary not available

## Medium Priority

- [x] **A-4: Split `doctor()` function** — 258 lines → 6 focused helpers
  - `check_git_version`, `check_worktree_accessibility`, `check_uncommitted_changes`,
    `check_behind_base`, `check_merge_conflicts`, `print_summary`, `print_recommendations`

- [ ] **B-1: Centralize format strings** — `messages.rs` with ~10 functions
  - ~15 inline `format!()` calls → centralized message helpers

- [ ] **`format_age()` edge-case tests**
  - Boundary values: 0 seconds, exactly 1 day, leap-second edge cases

- [ ] **`gw merge --ai-review`** — AI code review before merge
  - AI analyzes all changes before merging to base
  - Generates summary and suggests improvements
  - Carried over from Python TODO

- [ ] **`gw new --with-context`** — enhanced AI context on session start
  - Pass base branch recent commits, active files, project structure to AI
  - Carried over from Python TODO

## Low Priority

- [ ] **Release workflow SHA256 checksums**
  - Add checksum file to GitHub Release artifacts

## Known Issues

No currently known issues.
