# gw Command Reference

Complete reference for all gw (git-worktree-manager) commands.

## Core Worktree Management

### `gw new <branch> [OPTIONS] [-- AI_TOOL_ARGS...]`
Create new worktree for feature branch. Trailing positional args after the
branch name (or after `--`) are forwarded verbatim to the AI tool
(claude/codex/gemini).
- `--path <PATH>` — Custom worktree path (default: `../<repo>-<branch>`)
- `--base <BASE>` — Base branch to create from (default: from config or auto-detect)
- `-T, --term <METHOD>` — Terminal launch method. Accepts canonical name (e.g., `tmux`, `wezterm-tab`) or alias (e.g., `t`, `w-t`). Supports `method:session-name` for tmux/zellij (e.g., `tmux:mywork`). Use `-T skip` (or aliases `none`/`noop`) to skip the AI tool launch. See Terminal Launch Methods section below.
- `--prompt <PROMPT>` — Initial prompt as a CLI string. Use `-` to read the prompt from stdin (e.g. `cmd | gw new br --prompt -`). Avoid `--prompt -` together with `-T <terminal>` — the spawned terminal may inherit a closed stdin.
- `--prompt-file <PATH>` — Read initial prompt from a file (recommended for multi-line / quoted content)
- `--no-env-forward` — Disable auto-forwarding of `<TOOL>_*` env vars (e.g. `CLAUDE_*`) from the parent shell into the spawned process.

Only one of `--prompt`, `--prompt-file` may be used per invocation.
`--prompt`/`--prompt-file` are mutually exclusive with trailing AI tool args
(both end up setting the AI tool's prompt — pick one).

### `gw rm [target] [OPTIONS]`
Delete one or more worktrees. With no target, removes the current worktree; with one or more targets, removes each. Use `-i` for the multi-select UI.
- `-i, --interactive` — Multi-select UI (mutually exclusive with positional targets)
- `--dry-run` — Show what would be removed without removing
- `-k, --keep-branch` — Keep the branch (only remove worktree directory)
- `-r, --delete-remote` — Also delete the remote branch
- `-f, --force` — Bypass the busy-detection gate (also passes `--force` to `git worktree remove`)
- `--no-force` — Don't pass `--force` to `git worktree remove` (still allows the busy gate to apply)

### `gw list`
List all worktrees in a rich, human-readable view with status indicators (active, clean, modified, stale).

### `gw ls`
Print all worktrees as TSV (one row per worktree, tab-separated columns: `worktree_id`, `branch`, `status`, `age`, `repo_root`, `path`). For scripts and pipelines.

### `gw resume [TARGET] [OPTIONS] [-- AI_TOOL_ARGS...]`
Resume AI work in a worktree. Auto-detects existing Claude sessions and uses
`--continue` (claude) / `--resume` (codex/gemini); the resume flag is always
re-injected even when forward args are present.
Target is resolved in order: exact worktree name → exact branch name → exact path.
- `-T, --term <METHOD>` — Terminal launch method (same format as `gw new`)
- `--no-env-forward` — Disable auto-forwarding of `<TOOL>_*` env vars.
- Trailing args (after the target, or after `--`) are forwarded verbatim to the AI tool.

## Maintenance

### `gw doctor`
Run a 5-check health audit: (1) git version, (2) worktree accessibility (no missing/orphaned dirs), (3) uncommitted changes across worktrees, (4) busy-worktree detection, (5) Claude Code integration (whether the gw skills and hooks are installed in this repo's `.claude/`). Use `--session-start --quiet` for hook-friendly single-line output.

### `gw setup-claude`
Project-local one-click install: writes skill files into
`.claude/skills/gw-delegate/` and `.claude/skills/gw-manage/`, then
registers three Claude Code hooks (PreToolUse Bash guard, WorktreeCreate,
WorktreeRemove) in `<repo>/.claude/settings.json`. Idempotent — re-running
only writes files whose content changed. Existing user hooks are preserved.

### `gw upgrade`
Check for updates and install latest version from GitHub Releases.

## Configuration

Edit `~/.config/git-worktree-manager/config.json` directly to change settings.
Key fields:
- `ai_tool.command` — AI tool preset name (`claude`, `claude-yolo`, `codex`, `no-op`, etc.) or any command
- `launch.method` — Default terminal launch method (e.g., `wezterm-tab`, `tmux`, `foreground`)
- `update.auto_check` — `true` or `false`

## Hooks

Lifecycle hooks are configured via `hooks.post_new` and `hooks.pre_rm` in
`~/.config/git-worktree-manager/config.json` or `.cwconfig.json`.

```json
{
  "hooks": {
    "post_new": "npm install",
    "pre_rm": "git stash"
  }
}
```

Precedence: a repo-local `.cwconfig.json` overrides the global
`~/.config/git-worktree-manager/config.json`, so you can set per-project hooks
without affecting other repos. Hooks run with the **worktree path** as the
working directory, so relative paths and commands like `cd ..` refer to the
worktree. Config lookup is **main-repo-aware**: even though worktrees live in
sibling directories (`../<repo>-<branch>`), `gw` resolves `.cwconfig.json` from
the main repo root, meaning a single `.cwconfig.json` at the main repo controls
hooks for all worktrees of that repo.

## Shell Integration

### `gw shell-setup`
Interactive setup for shell integration (gw-cd function).

### `gw-cd [branch]`
Shell function to navigate to worktree by branch name. Supports:
- `gw-cd` — interactive selector
- `gw-cd feature-x` — direct navigation
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
- `gw _complete-targets` — List valid completion targets (worktree names + branch names)
- `gw _path --list-branches` — List worktree branch names
