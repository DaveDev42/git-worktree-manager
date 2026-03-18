# claude-worktree (Rust)

CLI tool integrating git worktree with AI coding assistants. Rust rewrite of [claude-worktree](https://github.com/DaveDev42/claude-worktree) for single-binary distribution and instant startup.

## Install

Download the latest release binary from [Releases](https://github.com/DaveDev42/claude-worktree-rs/releases).

```bash
# macOS / Linux
chmod +x cw && sudo mv cw /usr/local/bin/

# Or build from source
cargo install --path .
```

## Quick Start

```bash
# Create a new worktree + launch AI tool
cw new fix-auth

# List worktrees
cw list

# Create PR
cw pr

# Resume work
cw resume fix-auth

# Merge back
cw merge
```

## Commands

| Command | Description |
|---------|-------------|
| `cw new <name>` | Create worktree + launch AI tool |
| `cw pr [branch]` | Create GitHub PR |
| `cw merge [branch]` | Rebase + merge + cleanup |
| `cw resume [branch]` | Resume AI session |
| `cw delete <target>` | Remove worktree |
| `cw list` | List all worktrees |
| `cw status` | Show current worktree info |
| `cw tree` | Visual tree display |
| `cw stats` | Usage analytics |
| `cw diff <b1> <b2>` | Compare branches |
| `cw sync [branch]` | Rebase on base branch |
| `cw config show/set/use-preset/reset` | Configuration |
| `cw backup [branch]` | Git bundle backup |
| `cw restore <branch>` | Restore from backup |
| `cw doctor` | Health check |
| `cw scan` | Register repos for global mode |
| `cw -g <cmd>` | Global mode (cross-repo) |

## Shell Integration

```bash
# bash/zsh
source <(cw _shell-function bash)

# fish
cw _shell-function fish | source
```

This enables `cw-cd <branch>` for quick worktree navigation.

## Configuration

Config file: `~/.config/claude-worktree/config.json` (compatible with Python version).

```bash
cw config use-preset claude       # Default
cw config use-preset claude-yolo  # Skip permissions
cw config use-preset codex        # OpenAI Codex
cw config use-preset no-op        # No AI tool
```

## License

BSD-3-Clause
