# TODO - git-worktree-manager (gw)

This document tracks remaining planned features and enhancements.

## Open

### Infrastructure / CI

- [ ] **Provision `RELEASE_PLEASE_TOKEN` repo secret.** The
  `release-please.yml` workflow now prefers a maintainer-supplied PAT
  (fine-grained: `Contents: r/w` + `Pull requests: r/w` on this repo;
  or classic with `repo`) and falls back to `GITHUB_TOKEN` when unset.
  With the PAT, release-please's push to the release-PR branch
  triggers `pull_request` CI normally; without it, the release PR
  stays BLOCKED on required status checks (e.g. PR #168 currently)
  until someone closes-and-reopens it. A single fine-grained PAT can
  be scoped to both this repo and `DaveDev42/nokhwa` (same issue
  there) — see `nokhwa/TODO.md` for the matching item.

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
