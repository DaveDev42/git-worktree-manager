//! Body of the `manage` skill — worktree management + health rulebook +
//! hook recommendation catalog.

pub fn content() -> &'static str {
    r#"---
name: manage
description: "Manage git worktrees safely across multiple parallel sessions. Auto-applies when the user invokes gw list/delete/clean/sync/merge/pr/resume."
allowed-tools: Bash, Read, Edit
---

# gw: manage

This skill helps you (Claude) operate the `gw` (git-worktree-manager) management
commands safely when the user is working across multiple parallel worktrees.
Use it whenever the user asks about listing, deleting, syncing, merging,
opening PRs, resuming sessions, or otherwise inspecting worktree state. It also
defines a health rulebook (problems to watch for) and a catalog of Claude Code
hooks you may suggest installing on the user's consent.

## 1. Command Guidance

These are the management-side commands. For full flag detail, see
`references/gw-commands.md` (auto-loaded next to this SKILL.md).

| Command | Purpose |
|---------|---------|
| `gw list` (alias `gw ls`) | List all worktrees with status indicators (active, clean, modified, stale). |
| `gw status` | Show detailed info about the current worktree. |
| `gw delete [target]` | Delete a worktree (and optionally its branch / remote branch). |
| `gw clean` | Batch cleanup of merged or stale worktrees by filter (`--merged`, `--older-than`); `--dry-run` previews. Use `gw delete -i` for interactive selection. |
| `gw sync [branch]` | Rebase a worktree (or `--all`) against its base branch. |
| `gw merge [branch]` | Merge a feature branch into its base branch. |
| `gw resume [branch]` | Resume an AI session in a worktree (auto-uses `--continue` when possible). |
| `gw shell [worktree] [cmd...]` | Open an interactive shell in a worktree, or run a one-off command there. |
| `gw diff <a> <b>` | Compare two branches (full / `--summary` / `--files`). |
| `gw change-base <new> [branch]` | Move a worktree onto a different base branch. |
| `gw backup create/list/restore` | Create or restore a `git bundle` backup of a worktree. |
| `gw stash save/list/apply` | Worktree-aware stash that survives across worktrees. |
| `gw tree` | Visual tree of the worktree hierarchy. |
| `gw stats` | Worktree count, age, on-disk size. |

`gw new` (creating a worktree to delegate work into) is owned by the sibling
`delegate` skill — defer to that skill for new-task workflows.

For the full flag matrix, terminal launch methods, config keys, and helper
commands, read `references/gw-commands.md`.

## 2. Worktree-Health Rulebook

These rules apply when you (Claude) help the user with worktree commands.
Each rule names a symptom, why it hurts, what healthy looks like, how to
detect it, and what to suggest. Apply them proactively but quietly — surface
a concern once per session per rule, then drop it unless the user asks.

### Rule: Stale cwd / externally-deleted worktree

**Symptom:** The current working directory disappears mid-session because
another `gw` session ran `gw delete` on this worktree (or the user removed
it manually from another terminal).

**Why it hurts:** Every subsequent shell, git, or tool call fails with
"No such file or directory". `git push`, `npm publish`, `cargo publish`,
test runs, even a plain `ls` all break. The user wastes time diagnosing
what looks like permission/path bugs but is really a missing cwd.

**Healthy state:** `pwd` resolves to a real directory, and
`git rev-parse --show-toplevel` succeeds and matches a registered worktree
(visible in `gw status` / `gw list`).

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
"worktree A is 12 commits behind main; want me to `gw sync` it before we
start?" If multiple worktrees lag, suggest `gw sync --all`.

### Rule: Don't kill busy siblings

**Symptom:** A delete or clean operation in the main repo session removes
a worktree that another Claude session is actively working in. The most
common path is volunteering `--force` to "clean up" after `gw delete` or
`gw clean` complains that a target is busy.

**Why it hurts:** The sibling session's review-fix loop or long-running
task halts mid-flight. Even if the work is on a pushed branch, local
state and any uncommitted changes are gone, and the sibling's cwd dies
underneath it.

**Healthy state:** `gw delete` and `gw clean` complete without `--force`
— i.e. they respect the built-in busy gate. `--force` is reserved for
cases where the user has explicitly said "force-delete this even if
busy". Never volunteer `--force` to make an error message go away.

**How to detect:** Before any `gw delete <target>` or `gw clean` call,
run `gw list` and check the candidates' busy badges. If a candidate is
busy, do **not** add `--force`.

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

## 3. Recommended-Hooks Catalog

When a rule's suggested action is to add a Claude Code hook, pick from
this catalog. Each entry includes the JSON to write, the dependency (the
`gw` helper command it invokes), and the rationale. Edit the **project's**
`.claude/settings.json` (NOT `~/.claude/settings.json`) on user consent.

### Hook 1 — SessionStart sanity (primary)

The default recommendation. At session start, runs a fast sanity check
on the current cwd: validates that the directory exists, that it's a
registered gw worktree, that the configured base branch is reachable.
Cost is roughly ~5ms. Output is a single summary line. Directly addresses
the dominant friction (publish/test commands blocked because cwd was
removed by a sibling session).

```jsonc
{
  "hooks": {
    "SessionStart": [
      { "matcher": "*", "hooks": [
        { "type": "command", "command": "gw doctor --session-start --quiet" }
      ]}
    ]
  }
}
```

### Hook 2 — PreToolUse guard (advanced)

Parses the inbound bash command from the hook payload via stdin. If the
command matches a risky pattern (`git push`, `gh release`, `npm publish`,
`cargo publish`, `bun publish`, `pnpm publish`) AND the cwd looks
unhealthy (missing, unregistered, or significantly behind base), the hook
blocks the tool call with a clear explanation. Suggest only after the
user has expressed interest in stronger pre-publish safety — this hook
runs on every Bash invocation, so the latency budget matters.

```jsonc
{
  "hooks": {
    "PreToolUse": [
      { "matcher": "Bash", "hooks": [
        { "type": "command", "command": "gw guard --tool-input -" }
      ]}
    ]
  }
}
```

### Hook 3 — Stop summary (optional)

Prints a single line of worktree state when a turn ends: "uncommitted N
files, base differs by X commits". Informational only — does not block
or modify anything. Mention only on direct request from the user; do not
volunteer it.

```jsonc
{
  "hooks": {
    "Stop": [
      { "matcher": "*", "hooks": [
        { "type": "command", "command": "gw status --on-stop --quiet" }
      ]}
    ]
  }
}
```

## 4. When to suggest, when to stop

Read this section before recommending any hook. It is addressed to you,
the in-session Claude.

- **Default behavior:** when running a worktree command and the
  SessionStart hook (Hook 1) is NOT present in the project's
  `.claude/settings.json`, suggest Hook 1 *once* in this session. If the
  user accepts, edit `.claude/settings.json` (project-local). If the user
  refuses, or the hook is already present in equivalent form, do NOT
  bring it up again in subsequent sessions — the file's contents are the
  implicit state record. Do not maintain a separate memory of "I asked
  already".
- **Hook 2** (PreToolUse guard): mention only after the user has
  expressed interest in stronger pre-publish safety after seeing Hook 1's
  value. Do not volunteer it unprompted on a fresh project.
- **Hook 3** (Stop summary): mention only on direct request.
- **NEVER** edit `~/.claude/settings.json` or
  `~/.claude/settings.local.json`. Always edit the project-local file at
  `.claude/settings.json` (creating it if absent).
- **NEVER** install a hook silently — always state what the hook does,
  show the JSON, and ask for explicit consent before writing.
- When applying changes to `.claude/settings.json`, **preserve any
  existing hooks the user has** — read the file, parse it, and add to
  the relevant `hooks.<Event>` array. Do not replace the file wholesale,
  do not overwrite unrelated keys, and do not deduplicate by removing
  hooks the user installed for other reasons.
- If `.claude/settings.json` exists but is malformed JSON, do not try to
  repair it silently — surface the parse error to the user and ask how
  they want to proceed.
"#
}

pub fn reference_content() -> &'static str {
    r#"# gw Command Reference

Complete reference for all gw (git-worktree-manager) commands.

## Core Worktree Management

### `gw new <branch> [OPTIONS]`
Create new worktree for feature branch.
- `-p, --path <PATH>` — Custom worktree path (default: `../<repo>-<branch>`)
- `-b, --base <BASE>` — Base branch to create from (default: from config or auto-detect)
- `--no-term` — Skip AI tool launch
- `-T, --term <METHOD>` — Terminal launch method. Accepts canonical name (e.g., `tmux`, `wezterm-tab`) or alias (e.g., `t`, `w-t`). Supports `method:session-name` for tmux/zellij (e.g., `tmux:mywork`). See Terminal Launch Methods section below.
- `--bg` — Launch AI tool in background
- `--prompt <PROMPT>` — Initial prompt as a CLI string (single-line, best for short prompts)
- `--prompt-file <PATH>` — Read initial prompt from a file (recommended for multi-line / quoted content)
- `--prompt-stdin` — Read initial prompt from standard input (for piping). Avoid combining with `-T <terminal>` — the spawned terminal may inherit a closed stdin.

Only one of `--prompt`, `--prompt-file`, `--prompt-stdin` may be used per invocation.

### `gw delete [target] [OPTIONS]`
Delete a worktree.
- `-k, --keep-branch` — Keep the branch (only remove worktree directory)
- `-r, --delete-remote` — Also delete the remote branch
- `--no-force` — Don't use --force flag
- `-w, --worktree` — Resolve target as worktree directory name
- `-b, --branch` — Resolve target as branch name

### `gw list`
List all worktrees with status indicators (active, clean, modified, stale). Alias: `gw ls`.

### `gw status`
Show detailed info about the current worktree.

### `gw resume [branch] [OPTIONS]`
Resume AI work in a worktree. Auto-detects existing Claude sessions and uses `--continue`.
- `-T, --term <METHOD>` — Terminal launch method (same format as `gw new`)
- `--bg` — Launch AI tool in background
- `-w, --worktree` — Resolve as worktree name
- `-b, --by-branch` — Resolve as branch name

### `gw shell [worktree] [COMMAND...]`
Open interactive shell in a worktree, or execute a command.
```bash
gw shell feature-x           # interactive shell
gw shell feature-x npm test  # run command
```

## Git Workflow

### `gw merge [branch] [OPTIONS]`
Merge feature branch into base branch.
- `-i, --interactive` — Interactive rebase
- `--dry-run` — Show what would happen
- `--push` — Push to remote after merge
- `--ai-merge` — Use AI to resolve merge conflicts
- `-w, --worktree` — Resolve as worktree name

### `gw sync [branch] [OPTIONS]`
Sync worktree with base branch (rebase).
- `--all` — Sync all worktrees
- `--fetch-only` — Only fetch without rebasing
- `--ai-merge` — Use AI to resolve conflicts
- `-w, --worktree` / `-b, --by-branch` — Target resolution

### `gw change-base <new-base> [branch] [OPTIONS]`
Change base branch for a worktree.
- `--dry-run` — Show what would happen
- `-i, --interactive` — Interactive rebase
- `-w, --worktree` / `-b, --by-branch` — Target resolution

### `gw diff <branch1> <branch2> [OPTIONS]`
Compare two branches.
- `-s, --summary` — Show statistics only
- `-f, --files` — Show changed files only

## Maintenance

### `gw clean [OPTIONS]`
Batch cleanup of worktrees by filter. At least one filter is required.
- `--merged` — Delete worktrees for branches already merged to base
- `--older-than <DURATION>` — Delete worktrees older than duration (e.g., `7d`, `2w`, `1m`)
- `--dry-run` — Preview without deleting
- `-f, --force` — Tear through busy worktrees

For interactive selection, use `gw delete -i`.

### `gw doctor`
Run health check: git version, worktree accessibility, uncommitted changes, behind-base detection, merge conflicts, Claude Code integration.

### `gw upgrade`
Check for updates and install latest version from GitHub Releases.

### `gw tree`
Display worktree hierarchy as a visual tree.

### `gw stats`
Show worktree statistics (count, age, size).

## Backup & Stash

### `gw backup create [branch] [--all]`
Create git bundle backup of worktree(s).

### `gw backup list [branch] [--all]`
List available backups.

### `gw backup restore <branch> [--path <PATH>] [--id <ID>]`
Restore worktree from backup.

### `gw stash save [message]`
Save changes to worktree-aware stash.

### `gw stash list`
List stashes organized by worktree/branch.

### `gw stash apply <target-branch> [-s <stash-ref>]`
Apply stash to a different worktree.

## Configuration

### `gw config show`
Show current configuration.

### `gw config list`
List all configuration keys with descriptions.

### `gw config get <KEY>`
Get a config value. Keys use dot notation (see Key Config Keys section below).

### `gw config set <KEY> <VALUE>`
Set a config value. Key-specific valid values:
- `ai_tool.command` — Preset name (`claude`, `claude-yolo`, `claude-remote`, `claude-yolo-remote`, `codex`, `codex-yolo`, `no-op`) or any command name
- `launch.method` — Any terminal launch method name or alias (see Terminal Launch Methods)
- `update.auto_check` — `true` or `false`

### `gw config use-preset <NAME>`
Use a predefined AI tool preset: `claude`, `claude-yolo`, `claude-remote`, `claude-yolo-remote`, `codex`, `codex-yolo`, `no-op`.

### `gw config list-presets`
List available presets.

### `gw config reset`
Reset configuration to defaults.

## Hooks

### `gw hook add <EVENT> <COMMAND> [--id <ID>] [-d <DESC>]`
Add a lifecycle hook.

### `gw hook remove <EVENT> <HOOK_ID>`
Remove a hook.

### `gw hook list [EVENT]`
List hooks.

### `gw hook enable/disable <EVENT> <HOOK_ID>`
Toggle hook on/off.

### `gw hook run <EVENT> [--dry-run]`
Manually run hooks for an event.

**Available events:** `worktree.pre_create`, `worktree.post_create`, `worktree.pre_delete`, `worktree.post_delete`, `merge.pre`, `merge.post`, `pr.pre`, `pr.post`, `resume.pre`, `resume.post`, `sync.pre`, `sync.post`

## Export / Import

### `gw export [-o <FILE>]`
Export worktree configuration to JSON.

### `gw import <FILE> [--apply]`
Import configuration (preview by default, `--apply` to apply).

## Global Mode

Add `-g` or `--global` to any command to operate across all registered repositories.

### `gw -g list`
List worktrees across all registered repos.

### `gw scan [--dir <DIR>]`
Scan for and register git repositories.

### `gw prune`
Clean up stale registry entries.

## Shell Integration

### `gw shell-setup`
Interactive setup for shell integration (gw-cd function).

### `gw-cd [branch]`
Shell function to navigate to worktree by branch name. Supports:
- `gw-cd` — interactive selector
- `gw-cd feature-x` — direct navigation
- `gw-cd -g feature-x` — global (across repos)
- `gw-cd repo:branch` — repo-scoped navigation

## Terminal Launch Methods

Used with `-T` flag on `gw new` and `gw resume`. Supports `method:session-name` for tmux/zellij (e.g., `tmux:mywork`, `z:task1`).

| Method | Alias | Description |
|--------|-------|-------------|
| `foreground` | `fg` | Block in current terminal |
| `detach` | `d` | Fully detached process |
| `iterm-window` | `i-w` | iTerm2 new window |
| `iterm-tab` | `i-t` | iTerm2 new tab |
| `iterm-pane-h` | `i-p-h` | iTerm2 horizontal pane |
| `iterm-pane-v` | `i-p-v` | iTerm2 vertical pane |
| `tmux` | `t` | tmux new session |
| `tmux-window` | `t-w` | tmux new window |
| `tmux-pane-h` | `t-p-h` | tmux horizontal pane |
| `tmux-pane-v` | `t-p-v` | tmux vertical pane |
| `zellij` | `z` | Zellij new session |
| `zellij-tab` | `z-t` | Zellij new tab |
| `zellij-pane-h` | `z-p-h` | Zellij horizontal pane |
| `zellij-pane-v` | `z-p-v` | Zellij vertical pane |
| `wezterm-window` | `w-w` | WezTerm new window |
| `wezterm-tab` | `w-t` | WezTerm new tab |
| `wezterm-tab-bg` | `w-t-b` | WezTerm new tab (background, no focus steal) |
| `wezterm-pane-h` | `w-p-h` | WezTerm horizontal pane |
| `wezterm-pane-v` | `w-p-v` | WezTerm vertical pane |

## Key Config Keys

| Key | Description | Default |
|-----|-------------|---------|
| `ai_tool.command` | AI tool name or preset | `claude` |
| `ai_tool.args` | Additional arguments | `[]` |
| `launch.method` | Default terminal method | `foreground` |
| `launch.tmux_session_prefix` | tmux session prefix | `gw` |
| `launch.wezterm_ready_timeout` | WezTerm ready timeout (secs) | `5.0` |
| `update.auto_check` | Auto-check for updates | `true` |

## Helper Commands (for scripting and completion)

These hidden commands output newline-separated values, useful for scripting:
- `gw _config-keys` — List all config key names
- `gw _term-values` — List all valid `--term` values (canonical + aliases)
- `gw _preset-names` — List all AI tool preset names
- `gw _hook-events` — List all valid hook event names
- `gw _path --list-branches [-g]` — List worktree branch names
"#
}
