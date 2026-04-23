# Safe AI Tool Spawn — Escape-Free Launcher Prompt Injection

> **Note (post-merge, 2026-04-23):** The emitted shell line is
> `gw _spawn-ai <path>` (no `exec` prefix). The original `exec`-prefixed
> form described below was changed so the launching shell survives the AI
> tool's exit and the terminal tab stays open. See PR #94.

**Status:** Draft
**Date:** 2026-04-22
**Related:** Recurring failures when launching Claude Code via `gw new -T w-t-b` (and other launchers) with prompts that contain shell metacharacters (quotes, `$`, backticks, backslashes, multi-line content, non-ASCII).

## Problem

Every launcher path except `git_ops::create_pr` eventually sends the AI tool command through a POSIX shell:

- `foreground`: `bash -lc <cmd>`
- `detached`: equivalent shell wrap
- `iterm` / `tmux` / `wezterm` / `zellij`: terminal CLI pastes text that the pane's shell parses

`ai_tools::launch_ai_tool` builds that line with `shell_quote_join` (`src/operations/ai_tools.rs:279`):

```rust
fn shell_quote_join(parts: &[String]) -> String {
    parts.iter().map(|p| {
        if p.contains(char::is_whitespace) || p.contains('\'') || p.contains('"') {
            format!("'{}'", p.replace('\'', "'\\''"))
        } else { p.clone() }
    }).collect::<Vec<_>>().join(" ")
}
```

This single-layer POSIX quoting is then re-embedded into:

- AppleScript `write text "..."` blocks in `iterm.rs`, which apply *no* AppleScript-layer escaping of `"`, `\`, `$`.
- `wezterm cli send-text --no-paste` streams that ultimately hit readline.
- tmux `send-keys` / zellij `action write-chars` with their own quoting rules.

Result: any prompt containing embedded `"`, `\`, heredoc-looking markers, multi-line content, or IME sequences silently corrupts before Claude sees it. The user-visible symptom is "claude started but with wrong text" or "command failed to launch" — varying by launcher. `w-t-b` is the most reproducible victim because wezterm's send-text timing amplifies it.

`--prompt-file` on `gw new` (`src/cli.rs:112`) sidesteps ingest, but the launcher still flattens the file contents back into a shell-quoted argv before sending to the pane, so the escape hazard returns at spawn time.

## Goals

- Make every interactive launcher path carry prompts to the AI tool with **zero shell escape surface** — no quoting of user content, regardless of launcher or AI tool.
- Preserve interactive sessions (do not degrade to `claude --print` / non-interactive modes).
- Apply uniformly across all presets: `claude`, `claude-yolo`, `claude-remote`, `claude-yolo-remote`, `codex`, `codex-yolo`, and any future tool.
- Leave the `git_ops::create_pr` path unchanged — it already spawns the AI tool via `Command::new().args()` without a shell.

## Non-Goals

- Removing `gw new --prompt-file`. It stays as a convenience entry point.
- Changing AI tool preset argv shape or flags.
- Adding a new IPC mechanism beyond a short-lived local file.
- Windows-first hardening beyond baseline correctness. Unix is the primary supported platform; Windows must not regress.

## Approach — `gw` as Self-Exec Wrapper

Replace the shell-quoted argv line with a fixed two-token command that references a temp file describing the real spawn:

**Before** (current):
```
claude --dangerously-skip-permissions '<user prompt, POSIX-quoted>'
```

**After**:
```
exec gw _spawn-ai /tmp/gw-spawn-<uuid>.json
```

The spec file contains the real argv verbatim. `gw _spawn-ai` reads the file, unlinks it, `chdir`s, and `execvp`s the AI tool — replacing the `gw` process. The shell only ever parses hardcoded ASCII tokens; user prompt bytes never touch a shell parser.

## Architecture

### New modules

- `src/operations/spawn_spec.rs` — `SpawnSpec` type, `materialize()` writer, `execute()` reader/exec.

### Modified files

- `src/operations/mod.rs` — export `spawn_spec`.
- `src/operations/ai_tools.rs` — replace `shell_quote_join` call with `spawn_spec::materialize`; delete `shell_quote_join`.
- `src/cli.rs` — add hidden `_spawn-ai <path>` subcommand.
- `src/main.rs` (or `lib::entrypoint`) — dispatch `_spawn-ai` to `spawn_spec::execute`; run stale-spec cleanup once at top of `main`.
- `src/operations/setup_claude.rs` — soften `--prompt-file` recommendation language now that `--prompt` is equally safe.

### Existing modules untouched

- All six launcher modules (`foreground.rs`, `detached.rs`, `iterm.rs`, `tmux.rs`, `wezterm.rs`, `zellij.rs`). They continue to receive an opaque `&str` command line.
- `git_ops::create_pr` — already shell-free.
- Preset definitions in `config.rs`.

## Spawn Spec Format

```json
{
  "version": 1,
  "argv": ["<AI tool command name or path>", "<flag>", "...", "<raw prompt>"],
  "cwd": "<absolute path to worktree>",
  "self_unlink": true
}
```

- `version` — pinned at 1; future additions (env, stdin redirect) bump this.
- `argv[0]` — the configured AI tool command (e.g. `claude`) as supplied by `config::get_ai_tool_command`. `execute()` invokes `execvp`, which resolves the name against the PATH of the `gw _spawn-ai` process. In practice that PATH is inherited from the pane shell that ran `exec gw _spawn-ai <path>`, which is the same environment the tool would have used pre-change — so discovery semantics are unchanged. We deliberately do not pre-resolve to an absolute path: it would couple spec-write time to AI tool install location and complicate container/remote dev setups where the tool resolves only within the pane environment.
- `argv[1..]` — flags and the raw user prompt, byte-for-byte. No escaping layer touches them.
- `cwd` — absolute worktree path. `execute()` `chdir`s here before `execvp`, defending against pane/shell cwd drift and send-text race conditions.
- `self_unlink` — always `true` in v1. `execute()` unlinks the spec file immediately after read, before `execvp`.

### Tmp file creation

- Path: `std::env::temp_dir()` via `tempfile::Builder` with prefix `gw-spawn-`, suffix `.json`, and 16 random bytes, producing `gw-spawn-<32 lowercase hex>.json`.
- `tempfile::Builder::tempfile_in` uses `O_CREAT|O_EXCL` + mode `0o600` on Unix in one atomic step; collision with an attacker-planted file is not possible.
- Permissions: `0o600` on Unix means prompt content, which may include tokens or secrets, is readable only by the invoking user.
- Windows: relies on default user-profile ACL on `%TEMP%`.
- Filename charset: lowercase hex + `-` + `.` only. The resulting path contains no characters that require shell quoting.

### The returned shell line

`materialize()` returns a single line to hand to the launcher:

```
exec gw _spawn-ai <path>
```

- `exec` causes `bash -lc` / pane shells to replace themselves with `gw _spawn-ai`, minimizing process hops and ensuring the shell exits cleanly when the AI tool exits.
- `<path>` is emitted unquoted when it matches `^[A-Za-z0-9_/.:\\-]+$` (the common case). Otherwise — e.g. if `%TEMP%` on Windows resolves to a directory with spaces — it is wrapped in double quotes, which is safe under both `bash -lc` and `cmd /C` for paths that contain neither `"` nor `$` (and ours do not).

## `_spawn-ai` Subcommand

Hidden via clap `#[command(hide = true)]`. Not part of the public CLI surface.

Flow:

1. Read `<path>`, parse `SpawnSpec` (reject unknown `version`).
2. If `self_unlink`, `fs::remove_file(path)` immediately — best-effort, continue on error.
3. `std::env::set_current_dir(cwd)`. Fatal on failure.
4. Unix: `CommandExt::exec(Command::new(&argv[0]).args(&argv[1..]))`. `exec` returns only on error.
5. Windows: `Command::new(&argv[0]).args(&argv[1..]).status()` → propagate the child's exit code as `gw _spawn-ai`'s exit code.
6. On any failure, write a one-line diagnostic to stderr and `exit(127)` (shell convention for "command not found / could not start").

## Cleanup — Three-Layer Defense

1. **Primary**: `_spawn-ai` unlinks the spec before exec. Covers the normal path.
2. **Crash fallback**: every `gw` invocation runs a single best-effort pass at startup that removes any `gw-spawn-*.json` in `temp_dir()` whose `mtime` is older than 24 hours. Runs before subcommand dispatch, does not block on failure, silent on success.
3. **OS fallback**: platform-level temp cleanup. Last-resort safety net.

In practice a spec file outliving its spawn requires `gw _spawn-ai` to crash between file read and unlink — a very narrow window. The 24-hour sweep bounds worst-case residue.

## Error Handling

| Failure | Location | Response |
|---|---|---|
| Spec file write fails (disk full, TMPDIR missing) | `materialize()` | Return `CwError::Io`. Launcher never runs; prompt not lost (still in memory until caller drops it). |
| Launcher can't send text (e.g. wezterm not running) | existing paths | Unchanged. The spec file will be swept by the 24h pass. |
| JSON parse or version mismatch | `_spawn-ai` | stderr diagnostic, `exit(127)`. |
| `execvp` / Windows spawn fails | `_spawn-ai` | stderr diagnostic including the attempted argv[0], `exit(127)`. |
| `chdir(cwd)` fails | `_spawn-ai` | stderr diagnostic, `exit(127)`. Spec file already unlinked. |

## Testing

### Unit (`src/operations/spawn_spec.rs`)

- Round-trip serialize/deserialize for a fixture set of 12 prompts including:
  - `Fix the bug where user can "escape" quotes`
  - `$(rm -rf /) — literal, not an expansion`
  - `한글 테스트 🚀 ${PATH}`
  - Multi-line fake-heredoc: `\n<<'EOF'\nnot actually a heredoc\nEOF\n`
  - Backslash soup: `C:\Users\foo\bar \\path\\with\\backslashes`
  - A ~64 KB prompt (no NUL).
- `materialize()` shell line matches `^exec gw _spawn-ai (\S+|"[^"$\\]+")$`.
- Unix: generated file is mode `0o600`.
- `create_new` rejects duplicate path (simulate by pre-creating the target).
- `quote_path_for_shell`: paths containing `\` are double-quoted (Windows temp paths); pure ASCII Unix paths are emitted bare.

### Integration (`tests/spawn_roundtrip.rs`, new)

- Write a spec whose `argv[0]` is `/bin/echo` (or a tiny test helper that prints `argv[1..]` one per line), run `gw _spawn-ai <spec>`, capture stdout, assert byte-for-byte equality with the fixture prompt. This is the core regression guard.
- After `_spawn-ai` completes, assert the spec file no longer exists.
- Covers all fixture prompts from the unit list.

### Manual (documented in PR description)

Launcher matrix — smoke one fixture prompt through each:

- `gw new throwaway-fg -T fg --prompt "<killer prompt>"`
- `gw new throwaway-detach -T detach --prompt "..."`
- `gw new throwaway-iterm-t -T i-t --prompt "..."` (macOS)
- `gw new throwaway-tmux-w -T t-w --prompt "..."`
- `gw new throwaway-wez-tb -T w-t-b --prompt "..."` — **the reported failure; primary gate**
- `gw new throwaway-zellij-p -T z-p --prompt "..."`

Expected: each launches Claude (or whichever tool is configured) with the prompt byte-exact; focus behavior of each launcher unchanged.

## Rollout

- Single PR, `fix:` prefix, patch bump (per CLAUDE.md convention).
- Delete `shell_quote_join` and its tests in the same PR — no references remain.
- `setup_claude.rs` documentation switches from "Prefer `--prompt-file` to avoid escaping" to "Any of `--prompt`, `--prompt-file`, `--prompt-stdin` are safe; `--prompt-file` is still convenient for editor-managed multi-line content."
- No config migration, no user-visible flag changes.

## Open Questions / Future Work

- `version: 2` could add `env: {...}` for hook-injected variables that currently rely on inheritance. Not needed in v1.
- A `stdin_file` field could support tools that truly want stdin-fed prompts. Not needed for Claude/Codex today.
- If a future AI tool ships a true daemon/IPC for prompt handoff, prefer that over even the spec-file path.
