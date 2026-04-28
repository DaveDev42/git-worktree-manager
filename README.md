# git-worktree-manager (gw)

[![crates.io](https://img.shields.io/crates/v/git-worktree-manager.svg)](https://crates.io/crates/git-worktree-manager)
[![CI](https://github.com/DaveDev42/git-worktree-manager/actions/workflows/test.yml/badge.svg)](https://github.com/DaveDev42/git-worktree-manager/actions)
[![License: BSD-3-Clause](https://img.shields.io/badge/License-BSD%203--Clause-blue.svg)](LICENSE)

CLI tool integrating git worktree with AI coding assistants. Single static binary (~1.9MB), instant startup (~3ms).

Supports macOS (ARM64/x86), Linux (ARM64/x86), and Windows (x86_64).

Successor to [claude-worktree](https://github.com/DaveDev42/claude-worktree) (Python).

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

# Create and pass an initial prompt to the AI tool (short one-liner)
gw new fix-auth --prompt "Fix the JWT token expiration bug in auth.rs"

# Or read the prompt from a file (recommended for multi-line / quoted content)
gw new fix-auth --prompt-file /tmp/task.md

# Or pipe it from another command
generate-spec | gw new fix-auth --prompt-stdin
```

> **Note:** avoid combining `--prompt-stdin` with `--term` — the spawned
> terminal may inherit a closed stdin and behave unpredictably.

```bash
# List all worktrees
gw list

# Resume an AI session in an existing worktree
gw resume fix-auth

# Create a GitHub PR
gw pr

# Merge back to base branch and clean up
gw merge
```

## Commands

| Command | Description |
|---------|-------------|
| `gw new <name>` | Create worktree + launch AI tool |
| `gw resume [branch]` | Resume AI session in worktree |
| `gw shell [branch]` | Open shell in worktree |
| `gw pr [branch]` | Create GitHub PR |
| `gw merge [branch]` | Rebase + merge + cleanup |
| `gw delete [target...]` | Remove one or more worktrees (use `-i` for multi-select, `--dry-run` to preview) |
| `gw list` | List all worktrees |
| `gw status` | Show current worktree info |
| `gw tree` | Visual tree display |
| `gw stats` | Usage analytics |
| `gw diff <b1> <b2>` | Compare branches |
| `gw sync [branch]` | Rebase on base branch |
| `gw change-base <new-base> [branch]` | Change base branch for worktree |
| `gw clean` | Batch cleanup (`--merged`, `--older-than`) |
| `gw backup create/list/restore` | Git bundle backup (`backup create --output <dir>` to override the default backups root) |
| `gw stash save/list/apply` | Worktree-aware stash |
| `gw hook add/remove/list/...` | Lifecycle hooks |
| `gw config ...` | Configuration management |
| `gw export` / `gw import` | Config export/import |
| `gw doctor` | Health check diagnostics |
| `gw upgrade` | Self-update to latest version |
| `gw scan` | Register repos for global mode |
| `gw prune` | Clean up stale registry entries |
| `gw setup-claude` | Install Claude Code plugin (delegate + manage skills) |
| `gw shell-setup` | Interactive shell integration setup |
| `gw -g <cmd>` | Global mode (cross-repo) |

## Terminal Launchers

Control how AI tools are launched with `--term` (or configure a default via `gw config set launch.method`):

```bash
gw new fix-auth --term tmux         # New tmux session
gw new fix-auth --term iterm-tab    # New iTerm tab
gw new fix-auth --term zellij       # New Zellij session
gw new fix-auth --term wezterm-tab  # New WezTerm tab
gw new fix-auth --bg                # Force background variant of the launcher
gw resume fix-auth --bg             # Same modifier on resume
gw resume fix-auth --fg             # Force foreground variant (inverse)
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

### `--bg` / `--fg` modifiers

`gw new` and `gw resume` accept `--bg` and `--fg` as launcher modifiers that
swap to a paired variant of the resolved `--term` (or configured default):

| Base method   | `--bg` maps to    | `--fg` maps to |
|---------------|-------------------|----------------|
| `foreground`  | `detach`          | —              |
| `detach`      | —                 | `foreground`   |
| `wezterm-tab` | `wezterm-tab-bg`  | —              |
| `wezterm-tab-bg` | —              | `wezterm-tab`  |

Other launchers have no paired variant — `--bg`/`--fg` are silent no-ops in
that case (the original launcher runs unchanged). `--bg` and `--fg` cannot be
combined.

```bash
# Default config is wezterm-tab-bg → use --fg for a one-off foregrounded launch
gw resume fix-auth --fg

# Detach a foreground-default launch without editing config
gw new exploratory-spike --bg
```

## Claude Code Integration

Install the gw plugin into your Claude Code setup:

```bash
gw setup-claude    # One-time: installs the gw plugin to ~/.claude/plugins/gw/
```

The plugin bundles two skills:

- **`delegate`** — invoked via `/gw <task description>`. Spawns a new worktree and a Claude Code session inside it with the given task as the initial prompt. One-shot, fire-and-forget.
- **`manage`** — auto-applies when you (or Claude) run worktree management commands (`gw list/delete/clean/sync/merge/pr/resume`). Encodes a worktree-health rulebook (stale cwd, wrong-base branching, sibling drift, missing test/lint conventions) and a catalog of recommended Claude Code hooks. When relevant, Claude will *suggest* installing a hook into your project's `.claude/settings.json` and edit it on your consent — gw itself never modifies any settings file.

### Behavior changes in this release

- **`gw delete` no longer prompts.** Refusal messages now include a tiered explanation (Hard tier: active Claude session in the worktree, lockfile holder; Soft tier: process cwd scan). Pass `--force` once to override either tier — there is no interactive y/N path. Automation that piped `y` should switch to `--force`.
- **`gw delete` detects active Claude Code sessions.** A worktree with a Claude Code session that wrote a JSONL event in the last 10 minutes refuses delete, even if the session was started by another `gw new` call. This directly prevents the "second session deletes the worktree the first session is working in" failure mode. Override with `--force`.

## Shell Integration

```bash
# Interactive setup (recommended)
gw shell-setup
```

<details>
<summary>Manual setup</summary>

```bash
# bash/zsh - add to your shell rc file
source <(gw _shell-function bash)

# fish
gw _shell-function fish | source
```

</details>

This enables:
- **`gw-cd <branch>`** - Navigate to a worktree directory (interactive selector if no args)
- **Tab completion** - Branch names, config keys, and options

Generate shell completions separately with `gw --generate-completion <bash|zsh|fish|powershell|elvish>`.

## Configuration

Config file: `~/.config/git-worktree-manager/config.json` (also reads legacy `~/.config/claude-worktree/config.json`)

```bash
gw config show                     # Show current config
gw config list                     # List all keys with descriptions
gw config set <key> <value>        # Set a value
gw config get <key>                # Get a value
gw config reset                    # Reset to defaults
```

### AI Tool Presets

```bash
gw config use-preset claude              # Default
gw config use-preset claude-yolo         # Skip permission prompts
gw config use-preset claude-remote       # Remote control mode
gw config use-preset claude-yolo-remote  # Remote + skip permissions
gw config use-preset codex               # OpenAI Codex
gw config use-preset codex-yolo          # Codex without sandbox
gw config use-preset no-op               # No AI tool
```

### Environment Variables

| Variable | Description |
|----------|-------------|
| `CW_AI_TOOL` | Override AI tool command (space-separated) |
| `CW_LAUNCH_METHOD` | Override terminal launch method |

## Hooks

Run custom commands at lifecycle events. Pre-hooks abort the operation on failure.

```bash
gw hook add worktree.post_create "npm install"
gw hook add pr.pre "cargo test" --description "Run tests before PR"
gw hook list
gw hook disable worktree.post_create <hook-id>
```

**Available events:** `worktree.pre_create`, `worktree.post_create`, `worktree.pre_delete`, `worktree.post_delete`, `merge.pre`, `merge.post`, `pr.pre`, `pr.post`, `resume.pre`, `resume.post`, `sync.pre`, `sync.post`

Hook context is passed via `CW_*` environment variables.

## Notes

### Behavior change in `gw delete`

Since the multi-target `gw delete` change ([#100](https://github.com/DaveDev42/git-worktree-manager/pull/100)), single-target failures now exit with code `2` instead of `1`. This aligns with the new batch contract:

- `0` — full success, `--dry-run`, or interactive (`-i`) with no eligible worktrees / nothing selected
- `1` — user cancelled
- `2` — any target failed or was skipped (not found, busy, remove error)

Scripts treating exit code `1` as "delete failed" should switch to `!= 0` or specifically handle `2`. See `gw delete --help` for the full contract.

## License

BSD-3-Clause
