//! Body of the `delegate` skill — task delegation via `gw new`.

pub fn content() -> &'static str {
    r#"---
name: delegate
description: "Delegate coding tasks to isolated git worktrees. Invoke with: /gw <natural language task description>. Also handles worktree management: list, sync, clean, etc."
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

### Step 1.5: Pre-flight check

Run these three checks in order *before* invoking `gw new`. They are
procedural, not advisory — execute them the same way you execute parsing
in Step 1.

#### A.1 Stacked-base resolution

Determine where the new worktree should branch from.

```bash
git rev-parse --show-toplevel 2>/dev/null
git branch --show-current 2>/dev/null
```

- If cwd is the main repo or the default-base worktree (`main` / `master`
  / configured default): proceed with default base, omit `--base`.
- If cwd is inside a worktree whose branch is **not** the default base,
  ask the user one explicit question:
  > "You're currently inside the `<branch>` worktree. Should this new
  > task stack on `<branch>`, or branch from the default base
  > `<default>`?"
  - On "stack on `<branch>`" → pass `--base <branch>` to `gw new`.
  - On "default base" → omit `--base`.

Do not guess. The cost of asking once is small; the cost of a wrong base
is a manual rebase.

#### A.2 Duplicate-task detection

Cheap probes to avoid spawning a worktree for work that already exists.

```bash
gw list
# Only if `gh` is on PATH and the repo has a GitHub remote:
gh pr list --state all --search '<keyword>' --limit 5
```

Extract 2–3 high-signal keywords from the task description (the task-type
prefix you generated for the branch name plus the most distinctive noun
phrase). **Only confirm with the user if the match is unambiguous** — a
branch / PR title that contains the same task-type prefix AND a strong
noun match. Borderline matches are not surfaced; false-positive cost is
higher than the friction cost.

On unambiguous match, present options:
- continue anyway (spawn fresh)
- resume in the existing worktree (`gw resume <branch>`)
- cancel

#### A.3 Stale-cwd handling

If `git rev-parse --show-toplevel` errored with ENOENT or `pwd -P`
resolves to a path that no longer exists, the current shell is in a dead
worktree (likely deleted by another session).

This does **not** block spawn — `gw new` creates a fresh worktree
elsewhere — but you must:

- Tell the user clearly: "Current cwd `<path>` no longer exists — likely
  deleted by another session."
- After spawn succeeds, advise the user to `cd` into the new worktree (or
  the main repo) before running anything else from the dead shell.

### Step 2: Confirm and execute

All three prompt ingestion modes (`--prompt`, `--prompt-file`, `--prompt-stdin`) are equally safe from shell-escaping issues. Use `--prompt-file` for convenience when managing multi-line prompts in an editor or passing skill-generated files.

**Recommended (use this by default):**
```bash
# Write the full prompt to a temp file, then pass the path.
# `trap` ensures the file is cleaned up even if `gw new` fails.
trap 'rm -f /tmp/gw-prompt-$$.txt' EXIT
cat > /tmp/gw-prompt-$$.txt <<'PROMPT'
<task description — multi-line OK, quotes OK, no escaping needed>
PROMPT
gw new <branch-name> -T <terminal-method> --prompt-file /tmp/gw-prompt-$$.txt
```

**Short one-liner alternative:**
```bash
gw new <branch-name> -T <terminal-method> --prompt "<short task>"
```

**Piping from another command:**
```bash
generate-spec | gw new <branch-name> --prompt-stdin
```

Note: `--prompt-stdin` consumes the process's stdin. Avoid combining it with
`-T <terminal-method>` — the spawned terminal may inherit a closed stdin and
behave unpredictably. Use `--prompt-file` if you need to specify a terminal launcher alongside the prompt.

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
| `--prompt-file <path>` ⭐ | Convenient for multi-line prompts, editor-managed content, or skill-generated files. |
| `--prompt "<text>"` | Short single-line prompts only. |
| `--prompt-stdin` | Piping from another command (`cmd \| gw new ... --prompt-stdin`). |

Example (recommended):
```bash
trap 'rm -f /tmp/gw-prompt-$$.txt' EXIT
cat > /tmp/gw-prompt-$$.txt <<'PROMPT'
Fix JWT token expiration check in src/auth.rs.
Make sure to cover the "leeway" edge case and add a unit test.
PROMPT
gw new fix-auth -T w-t --prompt-file /tmp/gw-prompt-$$.txt
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
trap 'rm -f /tmp/gw-prompt-$$.txt' EXIT
cat > /tmp/gw-prompt-$$.txt <<'PROMPT'
Implement feature X
PROMPT
# `-T` omitted — uses the default launcher from `gw config get launch.method`.
gw new feature-x --prompt-file /tmp/gw-prompt-$$.txt
# ... work is done in the new worktree ...
gh pr create                       # create PR (run inside the worktree)
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

- Before running any `gw` subcommand that reads the current worktree (`gw status`, `gw list`, `gw delete` without an explicit target, etc.), make sure the shell's cwd still exists. If another session or an earlier `gw delete`/`gw clean` removed the worktree, the shell holds a stale pwd and commands behave unexpectedly. Quick guard:
  ```bash
  [ -d "$(pwd -P 2>/dev/null)" ] || { echo "FATAL: cwd missing (worktree likely deleted). Abort."; exit 1; }
  ```
  `gw new` does not need this guard (it creates a fresh worktree elsewhere); the guard matters for in-worktree commands.
- Use descriptive branch names: `fix-auth`, `feat-login-page`, `refactor-api`
- Specify base branch if not main/master:
  ```bash
  trap 'rm -f /tmp/gw-prompt-$$.txt' EXIT
  cat > /tmp/gw-prompt-$$.txt <<'PROMPT'
  <task description>
  PROMPT
  gw new fix-auth --base develop -T w-t --prompt-file /tmp/gw-prompt-$$.txt
  ```
- One focused task per worktree
- The delegated Claude Code instance works independently in its own worktree directory
- You can delegate multiple tasks in parallel to different worktrees
- **Fire-and-forget**: Once a worktree task is spawned, you CANNOT stop it, send follow-up messages, or interact with it. The initial prompt is the ONLY instruction the delegated instance receives. Therefore:
  - Make the prompt comprehensive — include all requirements, constraints, and acceptance criteria upfront
  - Use `--prompt-file` for complex or skill-generated prompts to manage them conveniently
  - If the user's request is vague or ambiguous, ask clarifying questions BEFORE spawning
  - Do NOT spawn a task assuming you can "correct course later" — you cannot
- For destructive worktree operations (`gw delete`, `gw clean`), defer to the `manage` skill — in particular the don't-kill-busy-siblings rule.
"#
}
