# TODO - git-worktree-manager (gw)

This document tracks remaining planned features and enhancements.

No pending items.

## Known Issues

- **`gw backup restore` creates a standalone repo, not a registered worktree.**
  The bundle is cloned into the target directory as an independent repository
  (`.git/` is a full directory, not the `gitdir:` pointer that worktrees use),
  so the restored tree never shows up in `gw list` or `git worktree list`. The
  "Restore complete!" message is misleading. Needs a design decision: either
  (a) treat restore as disaster recovery and document the standalone outcome,
  or (b) re-attach the bundle as a real worktree of the current repo.
  Reproduced on Windows 2026-04-21; applies to all platforms.

- **`gw pr` swallows `gh pr create` stderr on failure.**
  When `gh` exits non-zero (e.g. origin is not a GitHub remote, auth expired),
  the flow prints "Generating PR description with AI..." and then exits
  without surfacing gh's error. The branch-cleanup step that follows makes it
  look like the PR succeeded. Needs: surface `gh`'s stderr on non-zero exit
  and skip cleanup on failure. See `src/operations/git_ops.rs:177-199`.

- **`test_global_list_multiple_worktrees_same_repo` is flaky.**
  Shares the real `~/.config/git-worktree-manager/registry.json` with every
  other integration test that calls `gw new`. Under CI parallelism
  `gw -g list`'s auto-prune can race with another test's registration and
  omit the current test's repo from the output, so the `multi-a` assertion
  panics. Observed on PR #87 (2026-04-21) in a first CI pass; re-run on
  the same commit went green. Fix: give each integration-test binary an
  isolated config home (set `HOME` / `XDG_CONFIG_HOME` per test), or gate
  global-list tests behind a serialize mutex. Affects
  `tests/test_global_ops.rs:298`.
