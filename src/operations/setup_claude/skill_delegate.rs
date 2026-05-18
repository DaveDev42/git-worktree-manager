//! Body of the `delegate` skill — task delegation via Agent(isolation: "worktree").

pub fn content() -> &'static str {
    r#"---
name: delegate
description: "Delegate isolated coding tasks to git worktrees via Agent(isolation: worktree). Auto-applies when the user asks you to fix/build/refactor something that warrants an isolated branch."
allowed-tools: Task
---

# git-worktree-manager (gw) — Task delegation

When the user describes a task that warrants isolation (a fix, a feature, a
refactor — anything where you'd want a clean branch), spawn it as a subagent
in an isolated worktree using Claude Code's native `Agent` tool with
`isolation: "worktree"`.

`gw` registers `WorktreeCreate` / `WorktreeRemove` hooks at the project level
(via `gw setup-claude`), so any `Agent(isolation: "worktree")` call in this repo
is routed through `gw new`. That means:

- `.cwshare` files (e.g., `.env`) are copied into the new worktree automatically.
- The `post_new` hook configured in `.cwconfig.json` / global config
  (e.g., `npm install`, `cargo build`) runs before the subagent starts.
- The worktree appears in `gw list` and is managed via `gw rm`.

If the user reports that subagent worktrees don't have the expected shared files,
or that `post_new` isn't firing, they probably need to run `gw setup-claude` once
in this repo's root.

## When to delegate vs. work in-session

Delegate when:
- The task is well-scoped (clear inputs, clear acceptance criteria).
- It can run independently — the subagent has everything it needs in the initial
  prompt.
- The user is OK with a separate branch / worktree being created.

Work in-session (don't delegate) when:
- The task requires back-and-forth with the user.
- It touches the *current* worktree's state and shouldn't be branched off.
- The user explicitly asked you to make the change here.

If unsure, ask.

## Pre-flight: choosing the base

`Agent(isolation: "worktree")` defaults to branching off the current worktree's
HEAD. If you're in a non-default worktree, ask once:

> "You're currently inside `<branch>`. Should this new task stack on `<branch>`,
> or branch from `main`?"

(Surface this only when the cwd branch != default base. The cost of asking once
is small; the cost of a wrong base is a manual rebase later.)

## Duplicate-task detection (light pass)

Before spawning, a cheap probe:

```bash
gw list
```

If an existing worktree's branch name strongly matches the task (same prefix +
distinctive noun), surface it and ask:

- Continue anyway (spawn fresh)?
- Resume in the existing worktree (`gw resume <branch>`)?
- Cancel?

Don't surface borderline matches — false-positive friction is worse than the
occasional duplicate.

## Branch naming

When invoking the Agent, give the subagent a clear, comprehensive task prompt.
Claude Code's worktree hook will derive a branch name from the task, or you can
suggest one in the prompt.

Conventions if you suggest a branch name:
- Lowercase, hyphen-separated, max ~50 chars.
- Conventional prefixes: `fix-`, `feat-`, `refactor-`, `docs-`, `test-`, `chore-`.
- Strip filler words (the, a, an, for, in, on, etc.).
- Examples:
  - "Fix the JWT token expiration check in auth" → `fix-jwt-token-expiration`
  - "Add user avatar upload feature" → `feat-avatar-upload`
  - "Refactor the database connection pool" → `refactor-db-connection-pool`

## What you don't do

- Don't call `gw new` directly. The `Agent(isolation: "worktree")` path goes
  through `gw`'s WorktreeCreate hook automatically.
- Don't write `--prompt-file` shell wrappers. The Agent's prompt IS the task
  description; nothing else is needed.
- Don't assume you can send follow-up messages to a spawned subagent. Make the
  initial prompt comprehensive — include all requirements, constraints, and
  acceptance criteria upfront. If the user's request is vague, ask clarifying
  questions before spawning.

## Cleanup

After a delegated task ships (PR merged or abandoned), the worktree can be
removed with `gw rm <branch>`. The `manage` skill covers cleanup safety
(busy-detection, etc.) — defer to that skill for destructive operations.
"#
}
