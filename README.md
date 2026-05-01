# git-worktree-manager (gw)

[![crates.io](https://img.shields.io/crates/v/git-worktree-manager.svg)](https://crates.io/crates/git-worktree-manager)
[![CI](https://github.com/DaveDev42/git-worktree-manager/actions/workflows/test.yml/badge.svg)](https://github.com/DaveDev42/git-worktree-manager/actions)
[![License: BSD-3-Clause](https://img.shields.io/badge/License-BSD%203--Clause-blue.svg)](LICENSE)

`gw` 1.0 is a lean CLI tool that pairs git worktrees with AI coding assistants. Inspired by [mr (myrepos)](https://myrepos.branchable.com/), it uses cwd-based scope discovery — no global registry, no cross-repo flags — so commands do exactly what you expect relative to where you run them. Target resolution is strict and consistent: worktree name → branch name → path. Single static binary (~1.9MB), ~3ms startup, supports macOS (ARM64/x86), Linux (ARM64/x86), and Windows (x86_64).

Successor to [claude-worktree](https://github.com/DaveDev42/claude-worktree) (Python), rewritten in Rust.

> **Backward compatible:** The `cw` command is included as an alias. Existing `cw` workflows, `.cwshare`, and `.cwconfig.json` files work unchanged.

## Install

```bash
cargo install git-worktree-manager
```

This installs both `gw` and `cw` binaries.

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
# Create a worktree and launch your AI coding assistant
gw new fix-auth

# Create with a specific terminal launcher
gw new fix-auth --term tmux

# Pass an initial prompt to the AI tool
gw new fix-auth --prompt "Fix the JWT token expiration bug in auth.rs"

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

## Commands

| Command | Description |
|---------|-------------|
| `gw new <name>` | Create worktree + launch AI tool |
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

## Scope and Target Resolution

### Scope discovery

`gw run`, `gw list`, `gw ls`, and other scope-wide commands discover worktrees relative to the current working directory — walking up to find a `.cwconfig.json`-rooted scope, or walking down from cwd. There is no global registry and no cross-repo flags.

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

Control how AI tools are launched with `--term` on `gw new` (configurable default via `.cwconfig.json`):

```bash
gw new fix-auth --term tmux         # New tmux session
gw new fix-auth --term iterm-tab    # New iTerm tab
gw new fix-auth --term zellij       # New Zellij session
gw new fix-auth --term wezterm-tab  # New WezTerm tab
gw new fix-auth --no-term           # Skip AI tool launch entirely
```

| Launcher | Variants |
|----------|----------|
| **Foreground** | `foreground` (default) |
| **Detached** | `detach` |
| **iTerm** | `iterm-window`, `iterm-tab`, `iterm-pane-h`, `iterm-pane-v` |
| **tmux** | `tmux`, `tmux-window`, `tmux-pane-h`, `tmux-pane-v` |
| **Zellij** | `zellij`, `zellij-tab`, `zellij-pane-h`, `zellij-pane-v` |
| **WezTerm** | `wezterm-window`, `wezterm-tab`, `wezterm-tab-bg`, `wezterm-pane-h`, `wezterm-pane-v` |

Each launcher also has a short alias (e.g., `t` for tmux, `i-t` for iterm-tab).

## Claude Code Integration

Install the gw plugin into your Claude Code setup:

```bash
gw setup-claude    # One-time: installs the gw plugin to ~/.claude/plugins/gw/
```

The plugin bundles two skills:

- **`delegate`** — invoked via `/gw <task description>`. Spawns a new worktree and a Claude Code session inside it with the given task as the initial prompt. One-shot, fire-and-forget.
- **`manage`** — auto-applies when you (or Claude) run worktree management commands. Encodes a worktree-health rulebook and a catalog of recommended Claude Code hooks. When relevant, Claude will *suggest* installing a hook into your project's `.claude/settings.json` and edit it on your consent — gw itself never modifies any settings file.

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

Config is resolved in layers: built-in defaults ← `~/.config/git-worktree-manager/config.json` ← repo-local `.cwconfig.json`. Repo-local values take precedence. There are no `gw config` subcommands — edit the JSON files directly.

Also reads legacy `~/.config/claude-worktree/config.json` from the Python predecessor.

Example `.cwconfig.json`:

```json
{
  "default_base": "main",
  "ai_tool": "claude",
  "launch_method": "tmux",
  "hooks": {
    "post_new": "npm install",
    "pre_rm": "cargo test --quiet"
  }
}
```

## Hooks

Two lifecycle hooks are available, configured in `.cwconfig.json` (or the global config):

| Hook | Trigger |
|------|---------|
| `hooks.post_new` | After `gw new` creates a worktree |
| `hooks.pre_rm` | Before `gw rm` removes a worktree |

Hooks run as `sh -c <cmd>` with the worktree directory as the working directory. A non-zero exit aborts the operation.

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
