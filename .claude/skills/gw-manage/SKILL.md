---
name: manage
description: "Manage git worktrees safely across multiple parallel sessions. Auto-applies when the user invokes gw list/delete/resume."
allowed-tools: Bash, Read, Edit
---

# gw: manage

This skill helps you (Claude) operate the `gw` (git-worktree-manager) management
commands safely when the user is working across multiple parallel worktrees.
Use it whenever the user asks about listing, deleting, resuming
sessions, or otherwise inspecting worktree state. It also
defines a health rulebook (problems to watch for) and guidance on Claude Code
hook integration.

## 1. Command Guidance

These are the management-side commands. For full flag detail, see
`references/gw-commands.md` (auto-loaded next to this SKILL.md).

| Command | Purpose |
|---------|---------|
| `gw list` (alias `gw ls`) | List all worktrees with status indicators (active, clean, modified, stale). |
| `gw rm [target]` | Delete a worktree (and optionally its branch / remote branch). Use `-i` for interactive batch selection. |
| `gw resume [branch]` | Resume an AI session in a worktree (auto-uses `--continue` when possible). |

`gw new` (creating a worktree to delegate work into) is owned by the sibling
`delegate` skill — defer to that skill for new-task workflows. Use Claude Code's
`Agent(isolation: "worktree")` for delegation; `gw` provides the lifecycle
wrapper via hooks.

For the full flag matrix, terminal launch methods, config keys, and helper
commands, read `references/gw-commands.md`.

## 2. Worktree-Health Rulebook

These rules apply when you (Claude) help the user with worktree commands.
Each rule names a symptom, why it hurts, what healthy looks like, how to
detect it, and what to suggest. Apply them proactively but quietly — surface
a concern once per session per rule, then drop it unless the user asks.

### Rule: Stale cwd / externally-deleted worktree

**Symptom:** The current working directory disappears mid-session because
another `gw` session ran `gw rm` on this worktree (or the user removed
it manually from another terminal).

**Why it hurts:** Every subsequent shell, git, or tool call fails with
"No such file or directory". `git push`, `npm publish`, `cargo publish`,
test runs, even a plain `ls` all break. The user wastes time diagnosing
what looks like permission/path bugs but is really a missing cwd.

**Healthy state:** `pwd` resolves to a real directory, and
`git rev-parse --show-toplevel` succeeds and matches a registered worktree
(visible in `gw list`).

**How to detect:** At session start, or as soon as a command fails with an
ENOENT-shaped error, run:

```bash
pwd && git rev-parse --show-toplevel
```

If either errors with "No such file or directory" (or `pwd` prints a path
that no longer exists), the cwd is gone.

**Suggested action:** Tell the user clearly that the worktree directory has
been removed underneath this session. Then either `cd` to the main repo
checkout, or run `gw new <branch>` to recreate the worktree from the same
branch and resume there. Do not attempt further work from the dead cwd.

### Rule: Wrong-base branching

**Symptom:** The user spawns a new worktree with `gw new fix-X` from `main`
when an in-flight feature branch should have been the base.

**Why it hurts:** When the parent feature branch later merges, the new
branch's diff includes those merged changes a second time. The user has
to rebase or cherry-pick to salvage clean history, and PR review surfaces
unrelated changes that confuse reviewers.

**Healthy state:** Every `gw new` call passes `--base <correct-branch>`
when the intended parent is *not* `main` (or the configured default base).

**How to detect:** Before running `gw new`, ask: is this work building on
top of another in-flight feature, or directly on the default base? Check
`gw list` for active feature branches that might be the intended parent.
If the new task's description references work happening on another branch,
that other branch is probably the right base.

**Suggested action:** Confirm the intended base with the user, then pass
`--base <branch>` explicitly to `gw new`. When in doubt, ask once rather
than guessing.

### Rule: Sibling worktree drift (pull-style awareness)

**Symptom:** The user starts work in worktree A, but a sibling worktree B
on the same base merged yesterday and A doesn't know.

**Why it hurts:** A is now N commits behind base. The eventual merge or PR
will require a rebase, or worse, ship code that silently relies on stale
assumptions about sibling work.

**Healthy state:** When starting work, the sibling base is up-to-date or
there is a clear "I will rebase later" plan that the user has acknowledged.

**How to detect:** At session start, run `gw list`. For each worktree on
the same base, check the lag both directions:

```bash
git log --oneline base..HEAD | wc -l   # commits ahead of base
git log --oneline HEAD..base | wc -l   # commits behind base
```

A non-trivial "behind" count (say, > 5) means drift worth surfacing.

**Suggested action:** Surface the drift in the session greeting, e.g.
"worktree A is 12 commits behind main; want me to rebase it before we
start?" If there is drift, suggest running `git rebase` inside the worktree.

### Rule: Don't kill busy siblings

**Symptom:** A delete operation in the main repo session removes a
worktree that another Claude session is actively working in. The most
common path is volunteering `--force` to "clean up" after `gw rm`
complains that a target is busy.

**Why it hurts:** The sibling session's review-fix loop or long-running
task halts mid-flight. Even if the work is on a pushed branch, local
state and any uncommitted changes are gone, and the sibling's cwd dies
underneath it.

**Healthy state:** `gw rm` completes without `--force` — i.e. it
respects the built-in busy gate. `--force` is reserved for cases where
the user has explicitly said "force-delete this even if busy". Never
volunteer `--force` to make an error message go away.

**How to detect:** Before any `gw rm <target>` call, run `gw list`
and check the candidates' busy badges. If a candidate is busy, do
**not** add `--force`.

**Suggested action:**
- On busy: skip with a clear report — "`<branch>` is busy; skipped.
  Re-run with `--force` only if you want to override."
- Never volunteer `--force` to silence the busy gate. Wait for the user
  to say it explicitly.

### Rule: Test/lint convention gap

**Symptom:** The project's `CLAUDE.md` is missing or doesn't list how to
run tests, lint, build, or format-check for this stack.

**Why it hurts:** Every fresh session re-derives commands, sometimes
guessing wrong (running `npm test` in a Cargo project, calling `pytest`
when the project uses `uv run pytest`, etc.). The result is "Command
Failed" errors that look like real bugs but are environmental, and
wasted iteration time.

**Healthy state:** `CLAUDE.md` has a one-line entry for each of: test,
lint, build, fmt-check (skipping any genuinely not used).

**How to detect:** At session start, read `CLAUDE.md` (or
`cat CLAUDE.md 2>/dev/null` if uncertain it exists). If it doesn't
mention a test runner, lint command, or build command appropriate to
detected stack files (`Cargo.toml` → cargo, `package.json` → npm/bun/pnpm,
`pyproject.toml` → pytest/uv, `go.mod` → go, etc.), the gap is real.

**Suggested action:** Ask the user once, "What's the canonical
test/lint/build invocation for this project? I'll write it into CLAUDE.md
so future sessions don't re-derive." On consent, edit `CLAUDE.md` (do
not silently rewrite it).

## 3. Hook integration

`gw` ships three Claude Code hooks that integrate worktree lifecycle:
- **PreToolUse(Bash) guard** — blocks Bash commands when cwd is unhealthy.
- **WorktreeCreate** — routes `Agent(isolation: "worktree")` through
  `gw new`, so `.cwshare` files and `post_new` hooks fire for subagent
  worktrees.
- **WorktreeRemove** — runs the `pre_rm` hook (advisory) before the
  subagent worktree is cleaned up.

To install all three idempotently into the current repo's
`.claude/settings.json`:

```bash
gw setup-claude
```

Safe to re-run; existing user hooks are preserved. Suggest this command
*once* per session if the user is hitting friction that one of the three
hooks would resolve. Don't volunteer it unprompted on a fresh project.

Never edit `~/.claude/settings.json` (user-global) — `gw setup-claude`
only ever writes to the project-local `.claude/settings.json`.

## 4. When to suggest, when to stop

- When a rule's suggested action involves Claude Code hook integration
  (e.g., stale-cwd guard, automatic worktree lifecycle), suggest `gw
  setup-claude` *once* per session, then stop. If the user accepts, run
  it. If the user refuses, don't bring it up again — the file's contents
  in `.claude/settings.json` are the implicit state record.
- Never modify `.claude/settings.json` by hand for the three hooks
  `gw setup-claude` manages. Use the command.
- If `.claude/settings.json` is malformed JSON, surface the parse error
  to the user — `gw setup-claude` will refuse to overwrite a broken file.
- Other manual hook edits (project-specific custom hooks) are fine; you
  can still edit `.claude/settings.json` for those, just preserve the
  three entries `gw setup-claude` owns.
