# git-worktree-manager (gw)

[![crates.io](https://img.shields.io/crates/v/git-worktree-manager.svg)](https://crates.io/crates/git-worktree-manager)
[![CI](https://github.com/DaveDev42/git-worktree-manager/actions/workflows/test.yml/badge.svg)](https://github.com/DaveDev42/git-worktree-manager/actions)
[![License: BSD-3-Clause](https://img.shields.io/badge/License-BSD%203--Clause-blue.svg)](LICENSE)

`gw` is a lean git worktree manager. Inspired by [mr (myrepos)](https://myrepos.branchable.com/), it uses cwd-based scope discovery — no global registry, no cross-repo flags — so commands do exactly what you expect relative to where you run them. Target resolution is strict and consistent: worktree name → branch name → path.

AI coding-assistant integration is built in: `gw new` creates a worktree and launches your AI tool of choice in it (use `-T skip` to skip the launch). Trailing positional args after the branch name are forwarded verbatim to the AI tool — `gw new feat-x -- --model opus` runs `claude --model opus` inside the new worktree. Future AI-assisted operations (e.g. `--ai` on merge / rebase) layer on top of the same worktree primitives.

Single static binary (~1.9MB), ~3ms startup. Supports macOS (ARM64/x86), Linux (ARM64/x86), and Windows (x86_64).

Successor to [claude-worktree](https://github.com/DaveDev42/claude-worktree) (Python), rewritten in Rust.

> **Migration note:** the legacy `cw` binary alias and `cw-cd` shell function were removed in 1.0. Existing user data — `.cwshare`, `.cwconfig.json`, and `~/.config/claude-worktree/` — is still read transparently; only the binary/shell entry points changed. Update aliases or shell rc files that reference `cw` / `cw-cd` to use `gw` / `gw-cd`.

## Install

```bash
cargo install git-worktree-manager
```

This installs the `gw` binary.

<details>
<summary>Other installation methods</summary>

```bash
# Homebrew (macOS/Linux)
brew tap DaveDev42/tap
brew install git-worktree-manager

# cargo-binstall (pre-built binary, no compile)
cargo binstall git-worktree-manager

# Direct download
# https://github.com/DaveDev42/git-worktree-manager/releases/latest
```

</details>

After installing, run `gw upgrade` at any time to update to the latest version (self-replacing binary). Pass `--yes` to skip the prompt — required in non-TTY environments (CI, nested processes). Homebrew users should use `brew upgrade git-worktree-manager` instead.

## Quick Start

```bash
# Create a worktree (and launch your AI coding assistant in it)
gw new fix-auth

# Override the launcher for this invocation only (e.g., background WezTerm tab)
gw new fix-auth -T w-t-b

# Just the worktree, no AI tool (`-T noop` and `-T none` are aliases for `skip`)
gw new fix-auth -T skip

# Pass an initial prompt to the AI tool
gw new fix-auth --prompt "Fix the JWT token expiration bug in auth.rs"

# Read the prompt from stdin (Unix idiom — replaces the old --prompt-stdin flag)
some-cmd | gw new fix-auth --prompt -

# Forward extra args to the AI tool (everything after the branch name, or after `--`)
gw new fix-auth -- --model opus --resume

# Or read the prompt from a file (recommended for multi-line / quoted content)
gw new fix-auth --prompt-file /tmp/task.md

# List all worktrees (rich human-readable output)
gw list

# Resume an AI session in an existing worktree
gw resume fix-auth

# Launch AI tool in the current worktree (or a named one)
gw spawn
gw spawn fix-auth

# Remove a worktree (interactive multi-select, or by name)
gw rm fix-auth
gw rm -i

# Run a command in every worktree in scope
gw run -- cargo test

# Run a command in one specific worktree
gw exec fix-auth -- git status
```

## Forwarding args to the AI tool

`gw new`, `gw resume`, and `gw spawn` accept all of gw's own options *before* the
positional argument; everything after is forwarded verbatim to the underlying AI
tool (claude / codex / gemini). Use `--` to disambiguate when forwarded args
start with a hyphen:

```bash
gw new feat-x --base main -- --model opus --append-system-prompt "be terse"
gw resume feat-x -- --model haiku
gw spawn  feat-x --prompt "summarize the diff" -- --print  # ERROR: --prompt + forward args
```

`--prompt` / `--prompt-file` and forward args are mutually exclusive — both end
up setting the AI tool's prompt, so combining them would silently produce two
prompts. Pick one.

`gw resume` always re-injects the AI tool's resume flag (`--continue` for
claude, `--resume` for codex/gemini) even when forward args are present.

### Environment variables

By default, gw snapshots `<TOOL>_*` env vars at invocation time (e.g.
`CLAUDE_*` when the AI tool is claude) and re-injects them into the spawned
process. This matters for launchers like wezterm/iterm/tmux/zellij that go
through a daemon — without re-injection, the new tab inherits the daemon's env,
not the env of the shell that ran `gw`. Pass `--no-env-forward` to opt out.

```bash
CLAUDE_CONFIG_DIR=~/work-claude gw new feat-x
```

For one-off non-`<TOOL>_*` vars, use the shell — `FOO=bar gw new feat-x` works
for the foreground / detached launchers that inherit the parent env directly.

## Commands

| Command | Description |
|---------|-------------|
| `gw new <name>` | Create worktree (and launch AI tool, unless `-T skip`) |
| `gw resume [target]` | Resume AI session in a worktree |
| `gw spawn [target]` | Launch AI tool in an existing worktree (default: current) |
| `gw rm [targets...]` | Remove one or more worktrees (`-i` interactive, `--dry-run`, `--force`) |
| `gw list` | List all worktrees (rich, human-readable) |
| `gw ls` | Print all worktrees as TSV (for scripts) |
| `gw exec <target> -- <cmd>` | Run a command in one specific worktree |
| `gw run -- <cmd>` | Run a command in every worktree in scope |
| `gw guard` | Claude Code hook helper: allow or block inbound tool use |
| `gw doctor` | Run diagnostics (5 health checks) |
| `gw upgrade` | Check for updates / upgrade |
| `gw setup-claude` | Install Claude Code skill for worktree task delegation |
| `gw shell-setup` | Interactive shell integration setup |
| `gw config <list\|get\|set\|edit>` | View or edit gw config (global / `.cwconfig.json`) — see [Configuration](#configuration) |

## Scope and Target Resolution

### Scope discovery

`gw run`, `gw list`, `gw ls`, and other scope-wide commands discover worktrees relative to the current working directory: it walks up first to find a `.cwconfig.json`-rooted scope, falling back to a downward walk from cwd if none is found. There is no global registry and no cross-repo flags.

### Target resolution

When a command accepts a `[target]` argument, resolution is strict and ordered:

1. Exact worktree name
2. Branch name
3. Path

No ambiguity modes, no lookup-mode flags. The first match wins.

## `gw run` and `gw exec`

`gw run` executes a command in every worktree in scope. `gw exec` runs in exactly one.

```bash
# Run tests in all worktrees, 4 in parallel
gw run -j 4 -- cargo test

# Only worktrees whose name matches a glob
gw run --only 'feat-*' -- npm install

# Skip the main worktree
gw run --no-main -- cargo clippy

# Keep going even if some worktrees fail
gw run --continue-on-error -- cargo build

# Run in one specific worktree
gw exec fix-auth -- git log --oneline -5
```

`gw ls` emits TSV columns (`worktree_id`, `branch`, `status`, `age`, `repo_root`, `path`) for scripting:

```bash
gw ls | awk -F'\t' '{print $2, $6}'   # branch + path
```

## Terminal Launchers

The launcher used by `gw new` / `gw spawn` / `gw resume` is resolved from the highest-priority source available, in this order:

| Priority | Source | Example |
|----------|--------|---------|
| 1 (highest) | `-T, --term <METHOD>` CLI flag | `gw new fix-auth -T w-t-b` |
| 2 | `CW_LAUNCH_METHOD` env var | `CW_LAUNCH_METHOD=iterm-tab gw new fix-auth` |
| 3 | repo-local `.cwconfig.json` `launch.method` | `{ "launch": { "method": "tmux" } }` |
| 4 | global `~/.config/git-worktree-manager/config.json` `launch.method` | (same JSON shape) |
| 5 (default) | built-in default | `foreground` |

Use `-T skip` (or its aliases `-T none` / `-T noop`) to skip the AI tool launch entirely. The `-T` value accepts any canonical method name or alias (see table below); for tmux/zellij, append `:<session-name>` to name the session (e.g., `-T tmux:mywork`).

| Launcher | Variants |
|----------|----------|
| **Foreground** | `foreground` (default) |
| **Detached** | `detach` |
| **iTerm** | `iterm-window`, `iterm-tab`, `iterm-pane-h`, `iterm-pane-v` |
| **tmux** | `tmux`, `tmux-window`, `tmux-pane-h`, `tmux-pane-v` |
| **Zellij** | `zellij`, `zellij-tab`, `zellij-pane-h`, `zellij-pane-v` |
| **WezTerm** | `wezterm-window`, `wezterm-tab`, `wezterm-tab-bg`, `wezterm-pane-h`, `wezterm-pane-v` |

New tabs/windows (`tmux-window`, `zellij-tab`, `wezterm-window`/`wezterm-tab`/`wezterm-tab-bg`) are named after the worktree directory so you can tell sessions apart at a glance.

## Claude Code Integration

Install gw's Claude Code integration into the current repo:

```bash
gw setup-claude    # Writes skill files + registers hooks into .claude/
```

This installs two skills and three hooks:

- **`gw-delegate`** — when you ask Claude to fix/build/refactor something that warrants isolation, Claude spawns a subagent in a new worktree via `Agent(isolation: "worktree")`. One-shot, fire-and-forget.
- **`gw-manage`** — auto-applies when you run worktree management commands. Encodes a worktree-health rulebook and hook-integration guidance.
- **Hooks**: PreToolUse(Bash) guard, WorktreeCreate (routes `Agent` calls through `gw new`), WorktreeRemove (runs `pre_rm` advisory). All registered into `.claude/settings.json`. Idempotent — safe to re-run.

## Shell Integration

```bash
# Interactive setup (recommended)
gw shell-setup
```

<details>
<summary>Manual setup</summary>

```bash
# bash/zsh — add to your shell rc file
source <(gw _shell-function bash)

# fish
gw _shell-function fish | source
```

</details>

This enables:
- **`gw-cd <target>`** — navigate to a worktree directory (interactive selector if no args)
- **Tab completion** — worktree names and branch names from the current scope, plus options

Generate shell completions separately with `gw --generate-completion <bash|zsh|fish|powershell|elvish>`.

## Configuration

Config is resolved in layers (later layers override earlier ones): built-in defaults → `~/.config/git-worktree-manager/config.json` → repo-local `.cwconfig.json`.

Also reads legacy `~/.config/claude-worktree/config.json` from the Python predecessor.

### `gw config`

A small surface for viewing and editing config without hand-rolling JSON:

| Command | What it does |
|---|---|
| `gw config list` | Show every known key with its resolved value and the scope it came from (`[global]`, `[repo]`, `[override: repo]`, `[default]`). |
| `gw config get <key>` | Print the resolved value of a single key (script-friendly: one line, exits non-zero when nothing is set). |
| `gw config set <key> <value>` | Write to global config. Add `--repo` to write a repo-local override to `<repo-root>/.cwconfig.json`. |
| `gw config edit` | Interactive TUI: scroll keys, `Tab` to swap between global and repo, `Enter` to edit, empty input to unset, `s` to save. |

Keys mirror the JSON paths in dot/kebab form: `ai-tool.command`, `ai-tool.args`, `ai-tool.guard`, `launch.method`, `launch.tmux-session-prefix`, `launch.wezterm-ready-timeout`, `update.auto-check`, `hooks.post-new`, `hooks.pre-rm`. Tab completion enumerates them.

Example `.cwconfig.json`:

```json
{
  "default_base": "main",
  "ai_tool": { "command": "claude", "guard": true },
  "launch": { "method": "tmux" },
  "hooks": {
    "post_new": "npm install",
    "pre_rm": "cargo test --quiet"
  }
}
```

`ai_tool.guard` (default `true`) injects a PreToolUse(Bash) hook into Claude sessions that gw launches, so any Bash tool call from a stale worktree (cwd no longer exists) is blocked with an instruction to stop and ask the user. The injection only affects sessions started by `gw new` / `gw spawn` / `gw resume`; user/project `~/.claude/settings.json` hooks are preserved and run alongside. Set `false` to disable.

## Hooks

Two lifecycle hooks are available, configured in `.cwconfig.json` (or the global config):

| Hook | Trigger |
|------|---------|
| `hooks.post_new` | After `gw new` creates a worktree |
| `hooks.pre_rm` | Before `gw rm` removes a worktree |

Hooks run as `sh -c <cmd>` with the worktree directory as the working directory.

- `pre_rm` non-zero exit logs a warning but does **not** block removal (advisory). The worktree is removed regardless. `--force` bypasses busy detection only — never this hook. _(Changed in v0.2.0: was blocking, now advisory, to align with Claude Code's WorktreeRemove hook which cannot block cleanup.)_
- `post_new` non-zero exit surfaces as a non-zero `gw new` exit code, but the worktree itself remains on disk because the hook runs after `git worktree add`. The AI tool launch is skipped. _(Still blocking — a failed `post_new` signals the worktree is not ready.)_

There are no `gw hook` CRUD subcommands — set hooks in the config file directly.

## Doctor

`gw doctor` runs 5 lean health checks: git version, worktree accessibility, uncommitted changes, busy worktrees, and Claude Code integration. Pass `--session-start` for hook-friendly non-interactive mode (single-line summary, always exits 0).

## `gw rm` Exit Codes

| Code | Meaning |
|------|---------|
| `0` | Full success, `--dry-run`, or `-i` with nothing selected / cancelled at selection UI |
| `1` | User cancelled at the confirmation prompt |
| `2` | Any target failed or was skipped (not found, busy, or remove error) |

Scripts should handle `!= 0` or specifically check for `2`.

## License

BSD-3-Clause
