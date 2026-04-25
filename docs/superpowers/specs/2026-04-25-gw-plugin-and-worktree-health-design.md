# gw Plugin Conversion + Worktree Health Skill + In-Use Detection Refinement

**Status:** Design
**Date:** 2026-04-25

## Context

Claude Code Insights Report (`~/.claude/usage-data/report.html`) over 469 sessions surfaces two patterns directly relevant to gw:

- **Command Failed: 1,816 events across 301 projects** — by far the largest tool-error bucket (Other 1,105, User Rejected 185, File Not Found 133). 8 of the top 20 most-failure-prone projects are worktree directories created by `gw new` (names contain `-feat-/-fix-/-chore-/-refactor-`).
- **"Environmental and workflow state confusion"** is called out as a top friction category: invalid cwd blocking publish, externally-deleted worktree interrupting a review-fix loop, branching from wrong base. The report itself notes "rare failures... are almost always **environmental**, not reasoning failures."

Cross-referencing with raw `session-meta` data confirms gw's worktrees are over-represented among failure-heavy sessions. Two failure modes recur:

1. A second Claude session deletes a worktree currently in use by a first session (cwd disappears mid-loop, every subsequent command fails).
2. A session has no in-context knowledge of project conventions (test/lint/build commands, base branch) and re-derives them imperfectly each time, leading to wrong commands.

gw today addresses neither. `gw setup-claude` installs a single skill at `~/.claude/skills/gw/` with command-reference content but no operational health guidance. `gw delete` does have busy detection (`busy.rs`, 623 lines), but its process cwd scan has known cross-platform fragility — the user reports it "isn't working well in practice."

## Goals

- Convert `gw setup-claude` from a single-skill installer to a Claude Code **plugin** installer, so multiple cohesive skills can be bundled and updated as a unit.
- Bundle a **worktree-health rulebook** into the management skill so Claude can proactively recommend hooks and surface concerns when the user runs risky worktree operations across multiple parallel sessions.
- Refine `gw delete`'s in-use detection to be **reliable on all OSes** by promoting a high-precision signal (Claude Code session activity from `~/.claude/projects/`) to the primary decision and demoting fragile process scans to a soft, advisory tier.
- Keep the gw CLI focused on worktree operations. Convention detection, settings.json editing, and user-facing recommendations live in the skill (executed by Claude in-session), not in the binary.

## Non-Goals

These are explicitly **out of scope** for this spec:

- `lsof +D`-style exhaustive cwd scanning (too slow, OS-divergent, false-positive prone).
- Tracking terminal/multiplexer pane lifetimes as in-use signal (18 launcher variants × OS-specific query APIs; deferred).
- Repository-scoped plugin installation (`gw setup-claude --local`); global only for now.
- Automatic installation of Claude Code hooks into `~/.claude/settings.json` or any project's `.claude/settings.json`. Hooks are *recommended* by the skill in-session; the user (assisted by Claude) decides and installs.
- Code-level auto-detection of project conventions (Cargo/package.json/pyproject heuristics inside gw). The skill guides Claude to ask the user instead.
- Push-style notifications between sibling worktrees (e.g. one session messaging another after a merge). Pull-style awareness — the next session reading sibling state via `gw list` — is in scope.
- Any change to existing worktree CRUD semantics other than the busy-detection refinement described below.

## Architecture

### Responsibility split

| Layer | Responsibility |
|---|---|
| **gw binary** | Worktree CRUD; in-use detection (decision logic); helper commands invokable from hooks (`gw doctor --session-start`, `gw guard --tool-input -`). Never edits any settings file. |
| **`gw setup-claude`** | Installs the gw plugin to `~/.claude/plugins/gw/`. Installs no hooks. Final output mentions that the bundled skill will recommend hooks in-session when appropriate. |
| **Plugin (skills)** | Encodes operational knowledge: when to use which gw command (`delegate`), and what healthy multi-worktree operation looks like including the catalog of recommended hooks (`manage`). |
| **Claude (in-session)** | Reads the skills, observes the actual project state, asks the user, edits the project's `.claude/settings.json` on user consent. |
| **User** | Final decision on every recommendation. The skill is designed so a refusal is recorded implicitly by the next session reading the now-present settings.json (no separate "declined" state needed). |

### Plugin layout

```
~/.claude/plugins/gw/
├── plugin.json              # plugin manifest (name: "gw", version, author)
└── skills/
    ├── delegate/
    │   └── SKILL.md         # gw new + --prompt-* guidance (the existing /gw flow)
    └── manage/
        ├── SKILL.md         # list/delete/clean/sync/merge/pr/resume guidance
        │                    # + worktree-health rulebook section
        │                    # + recommended-hooks catalog
        └── references/
            └── gw-commands.md  # full command reference (current `gw-commands.md`)
```

Plugin name `gw`. Skill IDs `gw:delegate` and `gw:manage`. The existing `/gw <task>` trigger phrase is preserved by `delegate`'s skill description (current behavior unchanged from user perspective).

### Migration

`gw setup-claude` removes any prior install at `~/.claude/skills/gw/` and `~/.claude/skills/gw-delegate/` (legacy) and installs the plugin at `~/.claude/plugins/gw/`. Idempotent: re-running with no content changes prints "already up to date."

## In-Use Detection (refined)

### Three-tier signal model

| Tier | Signal | Source | Decision strength |
|---|---|---|---|
| **Hard** | Active Claude Code session in the worktree | `~/.claude/projects/<encoded-path>/*.jsonl` event tail | Refuses delete; only `--force` overrides |
| **Hard** | Explicit lockfile | `lockfile.rs` (current mechanism, kept) | Same |
| **Soft** | Process cwd scan | `busy.rs` (refined, see below) | Refuses delete with a softer warning tone; same `--force` overrides |

### Hard tier 1: Claude session activity

`gw delete <worktree>` performs:

1. Encode `<worktree>`'s canonical absolute path using Claude Code's directory-encoding rule (replace `/` and `.` with `-`). Lookup `~/.claude/projects/<encoded>/`.
2. If the directory does not exist → no Claude session has ever run there → not active.
3. For each `*.jsonl` in that directory:
   - Read the **last ~200 lines** (bounded; jsonl events are small).
   - Walk lines from the end, parse JSON, skip events without a `timestamp` field (e.g. `permission-mode`, `last-prompt`, `file-history-snapshot` metadata trailers — verified empirically).
   - The first event with a `timestamp` is the most recent activity for that session.
   - Optionally validate the `cwd` field on that event matches the worktree (defends against path-encoding collisions).
4. If any session's most recent activity is **within 10 minutes**, treat the worktree as actively held by Claude.

**Rationale for choosing event timestamp over file mtime:** Empirical check during this design showed an active session's jsonl had mtime 3 hours stale because the file's tail consisted of metadata-only lines after the last real event. Parsing event timestamps is robust to this. mtime is unreliable for our purpose.

**Threshold of 10 minutes:** Long enough to cover normal user think/typing time and short tool waits; short enough that an idle/forgotten session releases the worktree without manual intervention. Hardcoded for v1; configurable later if needed.

### Hard tier 2: Lockfile

Existing `lockfile.rs` mechanism is kept unchanged. `gw shell` and `gw start` write explicit lockfiles; this remains the supported path for non-Claude tools that want to register an in-use claim.

### Soft tier: Process cwd scan (demoted + refined)

Currently `busy.rs` makes the decision *and* gathers diagnostics in one path. After this change:

- **Decision role: demoted in tone, not in effect.** Soft tier still refuses the delete (overridable by `--force`), but the message is a warning rather than a hard error. Hard tier carries the strong refusal language.
- **Diagnostic role: kept and improved.** Process scan results are shown in refusal messages so the user sees *which* processes might malfunction.

Refinements to `busy.rs`:

- Keep: self-process-tree exclusion, sibling exclusion, multiplexer-server exclusion.
- Add: TTY-presence flag per process (interactive vs background hint), process start time (so output can label "started 30s ago" — likely short-lived).
- ~~Remove: `is_suspicious_cmd` heuristic name-based filter.~~ **Correction during implementation:** investigation found `is_suspicious_cmd` is not a deny-list filter for refusals (as this section originally assumed) but a macOS-specific name-recovery fallback that calls `ps -o comm=` when `lsof` returns a version-string-shaped command name (e.g. `2.1.104`, `v1.2.3`). It is load-bearing for diagnostic display quality on macOS and was retained as-is.
- Cross-platform: macOS `lsof` and Linux `/proc` walks are kept as best-effort. On Windows, the Soft section is omitted entirely (Hard tier still works, decision quality unaffected).

Estimated code change: `busy.rs` 623 lines → ~230 lines (decision logic ~80, diagnostics ~150).

### Override semantics

Single flag: **`--force`**. Overrides:

- Existing git-level "uncommitted changes" guard (current behavior).
- Soft tier (process cwd scan).
- Hard tier (Claude session, lockfile).

`--force-in-use` is **not** introduced. The user's accepted reasoning: refusal *messages* differ in tone strongly enough between Hard and Soft that two flags add cognitive cost without commensurate safety. A user who reads an "Active Claude session, last activity 2 minutes ago" message and still passes `--force` is making an informed choice.

### Refusal message shapes

**Soft only (warning tone):**

```
⚠ Worktree 'feature-x' may be in use:

  Processes with cwd in this worktree:
    PID 12345  zsh         (interactive shell)
    PID 67890  cargo test  (started 30s ago)

  These may malfunction if the worktree is deleted.
  Re-run with --force to delete anyway.
```

**Hard only (strong refusal):**

```
✗ Cannot delete worktree 'feature-x' — in use:

  Active Claude session
    last activity: 2 minutes ago
    session: 64778b29-acf0-44a1-8349-8e23b79cbc2e

  Use --force to delete anyway.
```

**Both (Hard primary, Soft as additional info):**

```
✗ Cannot delete worktree 'feature-x' — in use:

  Active Claude session
    last activity: 2 minutes ago
    session: 64778b29-acf0-44a1-8349-8e23b79cbc2e

  Additional processes with cwd in this worktree:
    PID 12345  zsh         (interactive shell)
    PID 67890  cargo test  (started 30s ago)

  Use --force to delete anyway.
```

**Pass-through (silent):** No output beyond the existing `✓ Worktree 'X' deleted.`

### Edge cases

| Case | Handling |
|---|---|
| `~/.claude/projects/` does not exist | Treated as "no Claude in use." Pass. |
| jsonl is 0 bytes or all metadata | Walks the bounded tail; if no `timestamp` event found, treats as inactive. |
| Path-encoding collision (different worktree mapping to same encoded dir) | `cwd` field on the jsonl event is compared to the worktree path; mismatch is ignored. |
| Anthropic changes jsonl location/format | Encoding and tail parse are isolated in one module; on parse failure, falls back to "inactive" (false-negative direction; user data not put at risk by false positives). |
| `gw clean --merged` over many worktrees | In-use ones are skipped with a one-line note per skipped item; not a bulk failure. |

## Plugin Skills

### `gw:delegate`

Equivalent to the current `/gw` skill body. Trigger phrase remains `/gw <task description>`. Body covers:

- `gw new` invocation (prompt-file recommended, prompt-stdin and prompt-string alternatives).
- Branch-name conventions, base-branch handling, terminal launcher selection.
- Fire-and-forget caveat (no follow-up to spawned session).

No behavioral change vs. today; only the file moves into the plugin layout.

### `gw:manage`

New skill. Three sections:

#### Section 1 — Command guidance

Quick-reference table for `list/status/delete/clean/sync/merge/pr/resume/shell/diff/change-base/backup/stash/tree/stats`. The bulk of detail lives in `references/gw-commands.md` (carried over).

#### Section 2 — Worktree-health rulebook

A catalog of operational rules. **Each rule follows a 5-part structure:**

1. **Symptom** — observable signal Claude can check
2. **Why it hurts** — the failure mode this prevents
3. **What healthy looks like** — desired end state
4. **How to detect** — concrete commands/files/state Claude inspects
5. **Suggested action** — what Claude should propose to the user

Initial rules (v1):

- **Stale cwd / externally-deleted worktree.** Claude detects when its working directory has been removed (e.g. by another gw session). Action: alert user, suggest moving to main repo or re-creating worktree.
- **Wrong-base branching.** When creating sub-branches, the wrong base (e.g. `main` instead of an active feature branch) leads to rebase-and-conflict recovery work. Action: confirm intended base before `gw new`.
- **Sibling worktree drift (pull-style awareness).** When the user starts work in worktree A, check if sibling worktrees on the same base are present and whether the base has advanced since they were created. Action: surface in session greeting; suggest `gw sync --all`.
- **Test/lint convention gap.** When CLAUDE.md is missing or lacks test/lint/build commands, every session re-derives them, sometimes wrongly. Action: ask the user once for the project's commands and offer to write them to CLAUDE.md.

This rulebook is **embedded in `manage`'s SKILL.md**, not a separate skill. Trigger is "user invokes a worktree command" — at which point Claude consults the rulebook for relevant rules. (No standalone `/health` invocation.)

#### Section 3 — Recommended-hooks catalog

When the rulebook indicates Claude should suggest a hook, it picks from this catalog. Each entry includes the hook JSON, the matching event, the rationale, and the gw helper command it depends on.

**Hook 1 — SessionStart sanity (primary):**

```jsonc
{
  "hooks": {
    "SessionStart": [
      { "matcher": "*", "hooks": [
        { "type": "command", "command": "gw doctor --session-start --quiet" }
      ]}
    ]
  }
}
```

Prints one line: cwd validity, current branch, base branch presence, worktree registration status. Cost: ~5ms. Effect: directly addresses the report's #1 friction example (publish blocked by invalid cwd).

**Hook 2 — PreToolUse guard (advanced):**

```jsonc
{
  "hooks": {
    "PreToolUse": [
      { "matcher": "Bash", "hooks": [
        { "type": "command", "command": "gw guard --tool-input -" }
      ]}
    ]
  }
}
```

Reads the tool input from stdin, parses the bash command, checks for risk patterns (`git push`, `gh release`, `npm publish`, `cargo publish`) and validates that cwd is a healthy worktree before allowing them. Exits non-zero to block when guard fails. Suggested only when the user opts into stronger protection.

**Hook 3 — Stop summary (optional):**

```jsonc
{
  "hooks": {
    "Stop": [
      { "matcher": "*", "hooks": [
        { "type": "command", "command": "gw status --on-stop --quiet" }
      ]}
    ]
  }
}
```

Prints a one-line worktree state summary at session end. Informational only.

The skill instructs Claude to:
- Default to suggesting Hook 1 when `.claude/settings.json` does not already include a SessionStart entry referencing gw.
- Mention Hook 2 only if the user expresses interest in stronger pre-publish safety after seeing Hook 1.
- Mention Hook 3 only on direct user request.
- On consent, edit the project's `.claude/settings.json` (not `~/.claude/settings.json`).
- On refusal or if the project's `.claude/settings.json` already contains an equivalent hook, do not re-prompt in subsequent sessions (state is implicitly captured by the file's contents).

## New gw Helper Commands

Two new commands are added to support hooks. They are usable standalone but designed for hook invocation.

### `gw doctor --session-start [--quiet]`

Short, hook-friendly variant of `gw doctor`. One-line output covering:

- cwd canonical path + existence
- current branch + base branch
- whether cwd is a registered gw worktree
- whether base is reachable

Exit 0 always (informational).

### `gw guard --tool-input -`

Reads Claude Code hook payload from stdin. Parses the `tool_input` field. If the tool is `Bash` and the command matches a risk pattern, validates worktree health (cwd exists, current branch known, not stale-base). Exits 0 to allow, non-zero with a stderr message to block.

Risk patterns are a small fixed set in v1: `git push`, `gh release`, `gh pr merge`, `npm publish`, `cargo publish`, `bun publish`, `pnpm publish`. Extending the set is a follow-up.

## Out of Scope (recap)

- `lsof +D`-style write-mode-aware cwd scan
- Terminal/multiplexer pane lifetime tracking
- Repository-scoped plugin (`--local`)
- Automatic hook installation by gw
- Code-level project-convention auto-detection
- Push-style sibling notifications

## Implementation Work Units

The implementation plan should treat these as three loosely-coupled work units that can be reviewed independently:

### Unit 1 — In-use detection refinement (CLI only)

- New `claude_session.rs` module: encoding, jsonl tail parse, threshold check.
- Refactor `busy.rs`: demote process scan to soft tier, remove `is_suspicious_cmd`, add TTY/start-time fields, retain self/sibling/multiplexer exclusions.
- Update `gw delete` and `gw clean` to consume the new tiered API and emit refusal messages per shapes above.
- Tests: integration test cases for each tier and combination; unit tests for jsonl encoding and tail parse.

### Unit 2 — Plugin conversion (CLI only, no skill content change)

- Refactor `setup_claude.rs` to write to `~/.claude/plugins/gw/` with `plugin.json` manifest and skill subdirectories.
- Migration: remove legacy `~/.claude/skills/gw/` and `~/.claude/skills/gw-delegate/` on first run.
- Move existing skill content into `delegate/SKILL.md` unchanged (preserve `/gw` trigger phrase).
- Update `gw doctor` integration check to look for plugin path, with fallback to legacy path during transition.

### Unit 3 — `manage` skill + helper commands

- Author `manage/SKILL.md` with the three sections (command guidance, rulebook, hooks catalog).
- Implement `gw doctor --session-start --quiet`.
- Implement `gw guard --tool-input -`.
- Add tests for both new commands (unit + a small integration that pipes a sample hook payload).

Units 1 and 2 are independent. Unit 3 depends on Unit 2 (skill needs the plugin layout to land in).

## Success Criteria

- `gw setup-claude` installs the plugin and prints a one-line note that the bundled skill will recommend hooks in-session when appropriate.
- After a Claude session has been active in a worktree (any jsonl event with timestamp within 10 minutes), `gw delete` on that worktree refuses with the Hard-tier message regardless of OS.
- `gw delete` on a worktree where only an interactive shell is `cd`'d in (no Claude, no lockfile) refuses with the Soft-tier warning and passes with `--force`.
- `gw delete` on a worktree where nothing relevant is happening passes silently.
- The `manage` skill contains the rulebook and hook catalog; Claude in a fresh session of an active worktree project can produce a coherent suggestion to add Hook 1 to `.claude/settings.json` and edit it on user consent.
- `busy.rs` line count is reduced by ~60% and contains no Windows-divergent code in the decision path.
