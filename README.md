# git-worktree-manager (gw)

[![crates.io](https://img.shields.io/crates/v/git-worktree-manager.svg)](https://crates.io/crates/git-worktree-manager)
[![CI](https://github.com/DaveDev42/git-worktree-manager/actions/workflows/test.yml/badge.svg)](https://github.com/DaveDev42/git-worktree-manager/actions)
[![License: BSD-3-Clause](https://img.shields.io/badge/License-BSD%203--Clause-blue.svg)](LICENSE)

CLI tool integrating git worktree with AI coding assistants. Single static binary (~1.9MB), instant startup (~3ms).

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

## Quick Start

```bash
gw new fix-auth          # Create worktree + launch AI tool
gw list                  # List worktrees
gw pr                    # Create GitHub PR
gw resume fix-auth       # Resume AI session
gw merge                 # Merge back to base
```

## Commands

| Command | Description |
|---------|-------------|
| `gw new <name>` | Create worktree + launch AI tool |
| `gw pr [branch]` | Create GitHub PR |
| `gw merge [branch]` | Rebase + merge + cleanup |
| `gw resume [branch]` | Resume AI session |
| `gw shell [branch]` | Open shell in worktree |
| `gw delete <target>` | Remove worktree |
| `gw list` | List all worktrees |
| `gw clean` | Batch cleanup (`--merged`, `--older-than`) |
| `gw status` | Show current worktree info |
| `gw tree` | Visual tree display |
| `gw stats` | Usage analytics |
| `gw diff <b1> <b2>` | Compare branches |
| `gw sync [branch]` | Rebase on base branch |
| `gw config ...` | Configuration management |
| `gw backup create/list/restore` | Git bundle backup |
| `gw stash save/list/apply` | Worktree-aware stash |
| `gw hook add/remove/list/...` | Lifecycle hooks |
| `gw export` / `gw import` | Config export/import |
| `gw doctor` | Health check |
| `gw scan` | Register repos for global mode |
| `gw -g <cmd>` | Global mode (cross-repo) |

## Shell Integration

```bash
# bash/zsh
source <(gw _shell-function bash)

# fish
gw _shell-function fish | source

# Or use interactive setup
gw shell-setup
```

This enables `gw-cd <branch>` (and `cw-cd` alias) for quick worktree navigation.

## Configuration

Config file: `~/.config/claude-worktree/config.json`

```bash
gw config use-preset claude       # Default
gw config use-preset claude-yolo  # Skip permissions
gw config use-preset codex        # OpenAI Codex
gw config use-preset no-op        # No AI tool
```

## License

BSD-3-Clause
