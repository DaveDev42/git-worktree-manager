/// Claude Code skill installation for worktree task delegation.
use std::path::PathBuf;

use console::style;

use crate::constants::home_dir_or_fallback;
use crate::error::Result;

const SKILL_DIR: &str = "gw";
const LEGACY_SKILL_DIR: &str = "gw-delegate";
const SKILL_FILE: &str = "SKILL.md";
const REFERENCE_FILE: &str = "gw-commands.md";

/// Get the skill base directory.
fn skill_dir() -> PathBuf {
    home_dir_or_fallback()
        .join(".claude")
        .join("skills")
        .join(SKILL_DIR)
}

/// Get the skill installation path.
pub fn skill_path() -> PathBuf {
    skill_dir().join(SKILL_FILE)
}

/// Get the reference file path.
fn reference_path() -> PathBuf {
    skill_dir().join("references").join(REFERENCE_FILE)
}

/// Check if the Claude Code skill is already installed.
pub fn is_skill_installed() -> bool {
    skill_path().exists()
}

/// Write a file if its content differs from the new content.
/// Returns true if the file was written (created or updated).
fn write_if_changed(
    path: &PathBuf,
    new_content: &str,
) -> std::result::Result<bool, std::io::Error> {
    if path.exists() {
        let existing = std::fs::read_to_string(path).unwrap_or_default();
        if existing == new_content {
            return Ok(false);
        }
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, new_content)?;
    Ok(true)
}

/// Remove the legacy gw-delegate skill directory if it exists.
fn remove_legacy_skill() {
    let legacy_dir = home_dir_or_fallback()
        .join(".claude")
        .join("skills")
        .join(LEGACY_SKILL_DIR);
    if legacy_dir.exists() {
        let _ = std::fs::remove_dir_all(&legacy_dir);
    }
}

/// Install or update the Claude Code skill for worktree task delegation.
pub fn setup_claude() -> Result<()> {
    remove_legacy_skill();

    let skill = skill_path();
    let reference = reference_path();

    let skill_changed = write_if_changed(&skill, skill_content())?;
    let ref_changed = write_if_changed(&reference, reference_content())?;

    if !skill_changed && !ref_changed {
        println!(
            "{} Claude Code skill is already up to date.\n",
            style("*").green()
        );
        println!("  Location: {}", style(skill_dir().display()).dim());
        return Ok(());
    }

    let action = if !skill_changed && ref_changed {
        "updated"
    } else if skill.exists() && skill_changed {
        // File existed before we wrote (we just overwrote it)
        "updated"
    } else {
        "installed"
    };

    println!(
        "{} Claude Code skill {} successfully!\n",
        style("*").green().bold(),
        action
    );
    println!("  Location: {}", style(skill_dir().display()).dim());
    println!(
        "  Use {} in Claude Code to delegate tasks to worktrees.",
        style("/gw").cyan()
    );
    println!(
        "  Or just ask Claude about {} — it will use gw automatically.\n",
        style("worktree management").cyan()
    );

    Ok(())
}

fn skill_content() -> &'static str {
    r#"---
name: gw
description: "Delegate coding tasks to isolated git worktrees. Invoke with: /gw <natural language task description>. Also handles worktree management: list, sync, clean, PR, merge, etc."
allowed-tools: Bash
---

# git-worktree-manager (gw)

CLI tool integrating git worktree with AI coding assistants. Single binary, ~3ms startup.

## Natural Language Task Delegation

When the user invokes `/gw <task description>` (e.g., `/gw fix the auth token expiration bug`), follow these steps:

### Step 1: Parse the user's intent
From the natural language input, determine:
- **Task description** — the full task text (will be passed via `--prompt-file` in Step 2)
- **Branch name** — generate a short, descriptive branch name from the task (e.g., `fix-auth-token-expiration`). Use conventional prefixes: `fix-`, `feat-`, `refactor-`, `docs-`, `test-`, `chore-`.
- **Base branch** — use the default unless the user specifies otherwise

### Step 2: Confirm and execute

Prefer `--prompt-file` for anything beyond a single short line — it avoids all
shell escaping issues with quotes, newlines, and special characters.

**Recommended (use this by default):**
```bash
# Write the full prompt to a temp file, then pass the path.
cat > /tmp/gw-prompt-$$.txt <<'PROMPT'
<task description — multi-line OK, quotes OK, no escaping needed>
PROMPT
gw new <branch-name> -T <terminal-method> --prompt-file /tmp/gw-prompt-$$.txt
rm -f /tmp/gw-prompt-$$.txt
```

**Short one-liner alternative:**
```bash
gw new <branch-name> -T <terminal-method> --prompt "<short task>"
```

**Piping from another command:**
```bash
generate-spec | gw new <branch-name> -T <terminal-method> --prompt-stdin
```

Only one of `--prompt`, `--prompt-file`, `--prompt-stdin` may be given per invocation.

### Branch name rules
- Lowercase, hyphen-separated, max ~50 chars
- Strip filler words (the, a, an, for, in, on, etc.)
- Examples:
  - "Fix the JWT token expiration check in auth" → `fix-jwt-token-expiration`
  - "Add user avatar upload feature" → `feat-avatar-upload`
  - "Refactor the database connection pool" → `refactor-db-connection-pool`

### Terminal method selection
- **Default: omit the `-T` flag** to use the system default (`gw config get launch.method`). Only add `-T` if the user explicitly requests a specific terminal method.
- If the user explicitly asks for a specific method, use it. Common methods: `w-t` (WezTerm tab), `w-t-b` (WezTerm tab, background — no focus steal), `i-t` (iTerm2 tab), `t` (tmux session), `d` (detached/background)
- Once the user specifies a method, remember it for subsequent calls in the same session.

## Quick Reference

| Command | Description |
|---------|-------------|
| `gw new <branch> [--prompt-file <path> \| --prompt "..." \| --prompt-stdin]` | Create worktree + optionally launch AI with task |
| `gw delete <branch>` | Delete worktree and branch |
| `gw list` | List all worktrees with status |
| `gw status` | Show current worktree info |
| `gw resume [branch]` | Resume AI session in worktree |
| `gw pr [branch]` | Create GitHub Pull Request |
| `gw merge [branch]` | Merge branch into base |
| `gw sync [--all]` | Rebase worktree(s) onto base branch |
| `gw clean [--merged]` | Batch cleanup of worktrees |
| `gw diff <b1> <b2>` | Compare two branches |
| `gw change-base <new> [branch]` | Change base branch |
| `gw config <action>` | Configuration management |
| `gw doctor` | Run diagnostics |
| `gw tree` / `gw stats` | Visual hierarchy / statistics |
| `gw backup <action>` | Backup and restore worktrees |
| `gw stash <action>` | Worktree-aware stash management |
| `gw shell [worktree]` | Open shell in worktree |

## Delegate a task to a new worktree

Three ways to supply the initial prompt (mutually exclusive):

| Flag | When to use |
|------|-------------|
| `--prompt-file <path>` ⭐ | **Recommended.** Multi-line prompts, prompts with quotes/special chars, anything skill-generated. |
| `--prompt "<text>"` | Short single-line prompts only. |
| `--prompt-stdin` | Piping from another command (`cmd \| gw new ... --prompt-stdin`). |

Example (recommended):
```bash
cat > /tmp/gw-prompt-$$.txt <<'PROMPT'
Fix JWT token expiration check in src/auth.rs.
Make sure to cover the "leeway" edge case and add a unit test.
PROMPT
gw new fix-auth -T w-t --prompt-file /tmp/gw-prompt-$$.txt
rm -f /tmp/gw-prompt-$$.txt
```

Example (short form):
```bash
gw new fix-auth -T w-t --prompt "Fix JWT token expiration check"
```

### Terminal methods (use with -T flag)
- `w-t` — WezTerm new tab
- `w-t-b` — WezTerm new tab (background, no focus steal)
- `w-w` — WezTerm new window
- `i-t` — iTerm2 new tab
- `i-w` — iTerm2 new window
- `t` — tmux new session
- `t-w` — tmux new window
- `d` — detached (background, no terminal)

Use the method matching the user's terminal. If unsure, ask.

## Common Workflows

### Feature development
```bash
gw new feature-x --prompt "Implement feature X"
# ... work is done in the new worktree ...
gw pr feature-x                    # create PR
gw delete feature-x                # cleanup after merge
```

### Keep worktrees in sync
```bash
gw sync --all                      # rebase all worktrees onto their base
gw sync --all --ai-merge           # use AI to resolve conflicts
```

### Batch cleanup
```bash
gw clean --merged                  # delete worktrees for merged branches
gw clean --older-than 30d --dry-run  # preview old worktree cleanup
```

### Global mode (across repos)
```bash
gw -g list                         # list worktrees across all repos
gw -g scan --dir ~/projects        # discover repositories
```

## Guidelines

- Use descriptive branch names: `fix-auth`, `feat-login-page`, `refactor-api`
- Specify base branch if not main/master: `gw new fix-auth --base develop -T w-t --prompt-file /tmp/gw-prompt-$$.txt`
- One focused task per worktree
- The delegated Claude Code instance works independently in its own worktree directory
- You can delegate multiple tasks in parallel to different worktrees
- **Fire-and-forget**: Once a worktree task is spawned, you CANNOT stop it, send follow-up messages, or interact with it. The initial prompt is the ONLY instruction the delegated instance receives. Therefore:
  - Make the prompt comprehensive — include all requirements, constraints, and acceptance criteria upfront
  - Use `--prompt-file` for anything non-trivial so escaping does not silently corrupt the instructions
  - If the user's request is vague or ambiguous, ask clarifying questions BEFORE spawning
  - Do NOT spawn a task assuming you can "correct course later" — you cannot

## Full command reference

For detailed flags and options for all commands, see [gw-commands.md](references/gw-commands.md).
"#
}

fn reference_content() -> &'static str {
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
- `--prompt-stdin` — Read initial prompt from standard input (for piping)

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

### `gw pr [branch] [OPTIONS]`
Create GitHub Pull Request from worktree.
- `-t, --title <TITLE>` — PR title
- `-B, --body <BODY>` — PR body
- `-d, --draft` — Create as draft PR
- `--no-push` — Skip pushing to remote
- `-w, --worktree` / `-b, --by-branch` — Target resolution

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
Batch cleanup of worktrees.
- `--merged` — Delete worktrees for branches already merged to base
- `--older-than <DURATION>` — Delete worktrees older than duration (e.g., `7d`, `2w`, `1m`)
- `-i, --interactive` — Interactive selection
- `--dry-run` — Preview without deleting

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
