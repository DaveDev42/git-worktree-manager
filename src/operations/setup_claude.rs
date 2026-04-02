/// Claude Code skill installation for worktree task delegation.
use std::path::PathBuf;

use console::style;

use crate::constants::home_dir_or_fallback;
use crate::error::Result;

const SKILL_DIR: &str = "gw-delegate";
const SKILL_FILE: &str = "SKILL.md";

/// Get the skill installation path.
pub fn skill_path() -> PathBuf {
    home_dir_or_fallback()
        .join(".claude")
        .join("skills")
        .join(SKILL_DIR)
        .join(SKILL_FILE)
}

/// Check if the Claude Code skill is already installed.
pub fn is_skill_installed() -> bool {
    skill_path().exists()
}

/// Install or update the Claude Code skill for worktree task delegation.
pub fn setup_claude() -> Result<()> {
    let path = skill_path();
    let new_content = skill_content();

    let action = if path.exists() {
        let existing = std::fs::read_to_string(&path).unwrap_or_default();
        if existing == new_content {
            println!(
                "{} Claude Code skill is already up to date.\n",
                style("*").green()
            );
            println!("  Location: {}", style(path.display()).dim());
            return Ok(());
        }
        "Updated"
    } else {
        "Installed"
    };

    // Create parent directories
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(&path, new_content)?;

    println!(
        "{} Claude Code skill {} successfully!\n",
        style("*").green().bold(),
        action.to_lowercase()
    );
    println!("  Location: {}", style(path.display()).dim());
    println!(
        "  Use {} in Claude Code to delegate tasks to worktrees.",
        style("/gw-delegate").cyan()
    );
    println!(
        "  Or just ask Claude to parallelize work — it will use {} automatically.\n",
        style("gw").cyan()
    );

    Ok(())
}

fn skill_content() -> &'static str {
    r#"---
name: gw-delegate
description: Delegate coding tasks to isolated git worktrees using gw (git-worktree-manager). Use when the user wants to parallelize work, delegate a task to run in another branch, or split work across multiple Claude Code instances.
allowed-tools: Bash
---

# Worktree Task Delegation with gw

## Delegate a task to a new worktree

```bash
gw new <branch-name> -T <terminal-method> --prompt "<task description>"
```

Example:
```bash
gw new fix-auth -T w-t --prompt "Fix JWT token expiration check in src/auth.rs. The validation skips expiry."
```

This will:
1. Create a new git worktree on a new branch based on the current base branch
2. Open a new terminal (e.g. WezTerm tab)
3. Start Claude Code with the given prompt in interactive mode

### Terminal methods (use with -T flag)
- `w-t` — WezTerm new tab (recommended for WezTerm users)
- `w-w` — WezTerm new window
- `i-t` — iTerm2 new tab
- `i-w` — iTerm2 new window
- `t` — tmux new session
- `t-w` — tmux new window
- `d` — detached (background, no terminal)

Use the method matching the user's terminal. If unsure, ask.

## Check worktree status

```bash
gw list          # list all worktrees with status
gw status        # current worktree info
```

## After the delegated task is completed

```bash
gw pr <branch>              # create a GitHub Pull Request
gw merge <branch>           # merge the branch back
gw delete <branch>          # clean up worktree and branch
```

## Guidelines

- Use descriptive branch names: `fix-auth`, `feat-login-page`, `refactor-api`
- Specify base branch if not main/master: `gw new fix-auth --base develop -T w-t --prompt "..."`
- One focused task per worktree
- The delegated Claude Code instance works independently in its own worktree directory
- You can delegate multiple tasks in parallel to different worktrees
"#
}
