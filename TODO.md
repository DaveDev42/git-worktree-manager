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
