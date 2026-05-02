//! Body of the `/gw` slash command. Acts as a thin trigger that hands off
//! to the bundled `delegate` skill, which contains the actual task-routing
//! logic.

pub fn content() -> &'static str {
    r#"---
description: "Delegate a coding task to an isolated git worktree. Usage: /gw <natural-language task>."
allowed-tools: Bash
argument-hint: <task description>
---

The user invoked `/gw $ARGUMENTS`.

Use the `delegate` skill bundled with this plugin to handle the request.
The skill contains the full workflow for parsing the task, generating a
branch name, and spawning a fresh AI session in a new worktree via
`gw new ... --prompt-file ...`.

Brief recap (the delegate skill has the full version):

1. Parse the task description from `$ARGUMENTS`.
2. Generate a short, hyphenated branch name (`fix-`, `feat-`, etc.).
3. Write the full task into a temp prompt file and call:
   ```bash
   trap 'rm -f /tmp/gw-prompt-$$.txt' EXIT
   cat > /tmp/gw-prompt-$$.txt <<'PROMPT'
   <full task body>
   PROMPT
   gw new <branch-name> --prompt-file /tmp/gw-prompt-$$.txt
   ```
4. If the user did not specify a terminal launcher, omit `-T` (uses the
   default from `launch.method` in `~/.config/git-worktree-manager/config.json`).

If the task is ambiguous, ask one clarifying question before spawning —
the delegated session is fire-and-forget.
"#
}
