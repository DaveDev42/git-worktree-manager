# Safe AI Tool Spawn — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminate shell-escape failures when launching AI tools (Claude, Codex) with user prompts that contain quotes, `$`, backticks, backslashes, newlines, or non-ASCII by routing every launcher through a `gw _spawn-ai <spec-file>` self-exec wrapper.

**Architecture:** `ai_tools::launch_ai_tool` writes argv + cwd as JSON into a `tempfile::NamedTempFile`, emits a single shell line `exec gw _spawn-ai <path>`, and hands it unchanged to every launcher. The hidden `_spawn-ai` subcommand reads the spec, unlinks the file, `chdir`s, and `execvp`s the AI tool. Shell parsers only see ASCII hardcoded tokens and a filesystem-safe path — the raw prompt bytes never traverse a shell parser.

**Tech Stack:** Rust (edition 2021), `serde` / `serde_json`, `tempfile` (already a dep), `clap` derive, `thiserror`, `std::os::unix::process::CommandExt` (Unix exec), `libc` (Unix only). No new crates.

**Spec:** `docs/superpowers/specs/2026-04-22-safe-ai-tool-spawn-design.md`

---

## File Structure

**New:**
- `src/operations/spawn_spec.rs` — `SpawnSpec` struct, `materialize()` writer, `execute()` reader/exec, `sweep_stale()` 24h cleaner.
- `tests/spawn_roundtrip.rs` — integration test invoking `gw _spawn-ai` with a fake `argv[0]`.

**Modified:**
- `src/operations/mod.rs` — register `spawn_spec`.
- `src/operations/ai_tools.rs` — remove `shell_quote_join`, call `spawn_spec::materialize` instead.
- `src/cli.rs` — add hidden `SpawnAi { spec: PathBuf }` variant (`_spawn-ai` on the CLI).
- `src/entrypoint.rs` — dispatch `Commands::SpawnAi` to `spawn_spec::execute`; run `spawn_spec::sweep_stale()` once before dispatch (outside the `is_internal` fast path — spec file leaks are rare and the sweep is bounded).
- `src/operations/setup_claude.rs` — soften `--prompt-file` recommendation language.

**Untouched (verified):**
- All launcher modules under `src/operations/launchers/`. They continue to take an opaque `&str`.
- `src/operations/git_ops.rs::create_pr` — already shell-free via `Command::new().args()`.

---

## Task 1: Scaffold `SpawnSpec` module with round-trip test

**Files:**
- Create: `src/operations/spawn_spec.rs`
- Modify: `src/operations/mod.rs`

- [ ] **Step 1: Register the new module**

In `src/operations/mod.rs`, add the line in alphabetical position (after `shell`):

```rust
pub mod spawn_spec;
```

- [ ] **Step 2: Create `src/operations/spawn_spec.rs` with the type and unit test**

```rust
//! Spawn-spec — safely launch AI tools without shell escape hazards.
//!
//! Prompts with quotes/$/backticks/newlines break when re-quoted through
//! AppleScript/wezterm/tmux send-text layers. Instead, `materialize` writes
//! argv+cwd to a temp file and returns `exec gw _spawn-ai <path>` as the
//! launcher command. `execute` reads the spec, unlinks it, chdir's, and
//! execvp's the real tool — the pane shell only ever parses ASCII.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::Result;

pub const SPEC_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SpawnSpec {
    pub version: u32,
    pub argv: Vec<String>,
    pub cwd: PathBuf,
    pub self_unlink: bool,
}

impl SpawnSpec {
    pub fn new(argv: Vec<String>, cwd: PathBuf) -> Self {
        Self {
            version: SPEC_VERSION,
            argv,
            cwd,
            self_unlink: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_preserves_killer_prompts() {
        let killers = [
            r#"Fix the bug where user can "escape" quotes"#,
            r#"$(rm -rf /) — literal, not an expansion"#,
            "한글 테스트 🚀 ${PATH}",
            "multi\nline\n<<'EOF'\nnot a heredoc\nEOF\n",
            r"C:\Users\foo\bar \\path\\with\\backslashes",
            "`backtick` and 'single' and \"double\"",
        ];
        for prompt in killers {
            let spec = SpawnSpec::new(
                vec!["claude".into(), "--print".into(), prompt.into()],
                PathBuf::from("/tmp/wt"),
            );
            let json = serde_json::to_string(&spec).unwrap();
            let back: SpawnSpec = serde_json::from_str(&json).unwrap();
            assert_eq!(spec, back, "round-trip mismatch for: {:?}", prompt);
            assert_eq!(back.argv[2], prompt);
        }
    }

    #[test]
    fn large_prompt_round_trips() {
        let big = "x".repeat(64 * 1024);
        let spec = SpawnSpec::new(vec!["claude".into(), big.clone()], PathBuf::from("/tmp"));
        let json = serde_json::to_string(&spec).unwrap();
        let back: SpawnSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(back.argv[1], big);
    }
}
```

- [ ] **Step 3: Run the new tests — expect compile + pass**

Run: `cargo test --lib spawn_spec`
Expected: 2 passed.

- [ ] **Step 4: Commit**

```bash
git add src/operations/mod.rs src/operations/spawn_spec.rs
git commit -m "feat(spawn_spec): scaffold SpawnSpec struct with round-trip test"
```

---

## Task 2: Implement `materialize` — write spec and return shell line

**Files:**
- Modify: `src/operations/spawn_spec.rs`

- [ ] **Step 1: Write failing tests for `materialize`**

Append to the `tests` module in `src/operations/spawn_spec.rs`:

```rust
    #[test]
    fn materialize_writes_spec_and_returns_exec_line() {
        let dir = tempfile::tempdir().unwrap();
        let spec = SpawnSpec::new(
            vec!["/bin/echo".into(), "hello \"world\"".into()],
            dir.path().to_path_buf(),
        );
        let (shell_line, spec_path) = materialize_in_dir(&spec, dir.path()).unwrap();

        assert!(shell_line.starts_with("exec gw _spawn-ai "));
        assert!(spec_path.exists());

        let loaded: SpawnSpec =
            serde_json::from_str(&std::fs::read_to_string(&spec_path).unwrap()).unwrap();
        assert_eq!(loaded, spec);
    }

    #[test]
    fn materialize_filename_is_shell_safe() {
        let dir = tempfile::tempdir().unwrap();
        let spec = SpawnSpec::new(vec!["/bin/true".into()], dir.path().into());
        let (line, _path) = materialize_in_dir(&spec, dir.path()).unwrap();

        // "exec gw _spawn-ai " + path. path must contain only safe chars OR
        // be wrapped in double quotes. Temp dir in tests may have unsafe chars;
        // we only assert the emitted line is one of those two shapes.
        let tail = line.strip_prefix("exec gw _spawn-ai ").unwrap();
        let quoted = tail.starts_with('"') && tail.ends_with('"');
        let bare_safe = tail
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '/' | '.' | '-' | ':' | '\\'));
        assert!(quoted || bare_safe, "unsafe tail: {:?}", tail);
    }

    #[cfg(unix)]
    #[test]
    fn materialize_file_is_mode_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let spec = SpawnSpec::new(vec!["/bin/true".into()], dir.path().into());
        let (_line, path) = materialize_in_dir(&spec, dir.path()).unwrap();

        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "expected 0600, got {:o}", mode);
    }
```

- [ ] **Step 2: Run tests — expect compile failure**

Run: `cargo test --lib spawn_spec`
Expected: FAIL — `materialize_in_dir` not defined.

- [ ] **Step 3: Implement `materialize` + `materialize_in_dir`**

Add to `src/operations/spawn_spec.rs` (outside the `tests` module):

```rust
use std::fs;
use std::io::Write;
use std::path::Path;

/// Write `spec` to a 0600 tempfile in the system temp dir and return
/// `(shell_line, spec_path)`. `shell_line` is safe to hand to any launcher.
pub fn materialize(spec: &SpawnSpec) -> Result<(String, PathBuf)> {
    materialize_in_dir(spec, &std::env::temp_dir())
}

/// Test seam — write into an explicit directory.
pub fn materialize_in_dir(spec: &SpawnSpec, dir: &Path) -> Result<(String, PathBuf)> {
    fs::create_dir_all(dir)?;

    // tempfile gives us a random name + O_CREAT|O_EXCL + mode 0600 on Unix.
    let named = tempfile::Builder::new()
        .prefix("gw-spawn-")
        .suffix(".json")
        .rand_bytes(16)
        .tempfile_in(dir)?;

    let json = serde_json::to_vec(spec)?;
    // Scope the write so we can persist the file path below.
    {
        let mut f = named.as_file();
        f.write_all(&json)?;
        f.flush()?;
    }

    // Persist — stop tempfile from auto-deleting on drop. `_spawn-ai` unlinks
    // it after reading, and the 24h sweep handles crash residue.
    let (_file, path) = named.keep().map_err(|e| e.error)?;

    let shell_line = format!("exec gw _spawn-ai {}", quote_path_for_shell(&path));
    Ok((shell_line, path))
}

/// Shell-safe rendering for a path we just created. Paths produced by
/// `tempfile_in(temp_dir())` normally contain only [A-Za-z0-9_/.-], but some
/// Windows `%TEMP%` expansions include spaces; in that case we wrap in double
/// quotes. Our own filename never contains `"`, `$`, or backslash-escaped
/// metacharacters, so double quotes are sufficient under both bash and cmd.
fn quote_path_for_shell(path: &Path) -> String {
    let s = path.to_string_lossy();
    let safe = s.chars().all(|c| {
        c.is_ascii_alphanumeric() || matches!(c, '_' | '/' | '.' | '-' | ':' | '\\')
    });
    if safe {
        s.into_owned()
    } else {
        format!("\"{}\"", s)
    }
}
```

- [ ] **Step 4: Run tests — expect pass**

Run: `cargo test --lib spawn_spec`
Expected: 5 passed.

- [ ] **Step 5: Run clippy — zero warnings**

Run: `cargo clippy --all-targets -- -D warnings`
Expected: success.

- [ ] **Step 6: Commit**

```bash
git add src/operations/spawn_spec.rs
git commit -m "feat(spawn_spec): implement materialize writing 0600 tempfile"
```

---

## Task 3: Implement `execute` — read spec, unlink, chdir, exec

**Files:**
- Modify: `src/operations/spawn_spec.rs`

- [ ] **Step 1: Write a failing test that drives execute via a subprocess wrapper**

Because `execute` calls `execvp` on Unix (replacing the current process), unit-testing it in-process is impossible. Add a minimal pure-logic test for the pre-exec stages (read + parse + version check) and defer the exec itself to the integration test in Task 6.

Append to the `tests` module in `src/operations/spawn_spec.rs`:

```rust
    #[test]
    fn read_spec_rejects_wrong_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.json");
        std::fs::write(
            &path,
            r#"{"version":999,"argv":["x"],"cwd":"/","self_unlink":false}"#,
        )
        .unwrap();
        let err = read_spec(&path).unwrap_err();
        assert!(format!("{err}").contains("unsupported spawn spec version"));
    }

    #[test]
    fn read_spec_rejects_empty_argv() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.json");
        std::fs::write(
            &path,
            r#"{"version":1,"argv":[],"cwd":"/","self_unlink":false}"#,
        )
        .unwrap();
        let err = read_spec(&path).unwrap_err();
        assert!(format!("{err}").contains("empty argv"));
    }

    #[test]
    fn read_spec_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let spec = SpawnSpec::new(
            vec!["/bin/echo".into(), "hi".into()],
            dir.path().to_path_buf(),
        );
        let path = dir.path().join("ok.json");
        std::fs::write(&path, serde_json::to_vec(&spec).unwrap()).unwrap();
        let loaded = read_spec(&path).unwrap();
        assert_eq!(loaded, spec);
    }
```

- [ ] **Step 2: Run — expect compile failure on `read_spec`**

Run: `cargo test --lib spawn_spec`
Expected: FAIL — `read_spec` not found.

- [ ] **Step 3: Implement `read_spec` and `execute`**

Add to `src/operations/spawn_spec.rs`:

```rust
use crate::error::CwError;

/// Parse a spec file, rejecting unsupported versions and empty argv.
pub fn read_spec(path: &Path) -> Result<SpawnSpec> {
    let bytes = fs::read(path)?;
    let spec: SpawnSpec = serde_json::from_slice(&bytes)?;
    if spec.version != SPEC_VERSION {
        return Err(CwError::Other(format!(
            "unsupported spawn spec version: {} (expected {})",
            spec.version, SPEC_VERSION
        )));
    }
    if spec.argv.is_empty() {
        return Err(CwError::Other("spawn spec has empty argv".into()));
    }
    Ok(spec)
}

/// Execute a spawn spec. On Unix, replaces the current process via execvp.
/// On Windows, spawns a child and propagates its exit code. Never returns
/// `Ok(())` on success on Unix (process is replaced); returns `Ok(())` only
/// on Windows when the child exits successfully.
pub fn execute(spec_path: &Path) -> Result<()> {
    let spec = read_spec(spec_path)?;

    if spec.self_unlink {
        // Best-effort — proceed even if unlink fails (e.g. already gone).
        let _ = fs::remove_file(spec_path);
    }

    std::env::set_current_dir(&spec.cwd).map_err(|e| {
        CwError::Other(format!(
            "spawn-ai: chdir to {} failed: {}",
            spec.cwd.display(),
            e
        ))
    })?;

    let program = &spec.argv[0];
    let args = &spec.argv[1..];

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = std::process::Command::new(program).args(args).exec();
        // exec only returns on failure.
        return Err(CwError::Other(format!(
            "spawn-ai: exec {} failed: {}",
            program, err
        )));
    }

    #[cfg(windows)]
    {
        let status = std::process::Command::new(program)
            .args(args)
            .status()
            .map_err(|e| CwError::Other(format!("spawn-ai: spawn {} failed: {}", program, e)))?;
        let code = status.code().unwrap_or(1);
        std::process::exit(code);
    }
}
```

- [ ] **Step 4: Run tests — expect pass**

Run: `cargo test --lib spawn_spec`
Expected: 8 passed.

- [ ] **Step 5: Run clippy**

Run: `cargo clippy --all-targets -- -D warnings`
Expected: success.

- [ ] **Step 6: Commit**

```bash
git add src/operations/spawn_spec.rs
git commit -m "feat(spawn_spec): implement execute reading spec and execvp-ing target"
```

---

## Task 4: Implement `sweep_stale` — 24h tempfile cleanup

**Files:**
- Modify: `src/operations/spawn_spec.rs`

- [ ] **Step 1: Write the failing test**

Append to the `tests` module:

```rust
    #[test]
    fn sweep_stale_removes_old_spec_files_only() {
        use std::time::{Duration, SystemTime};
        let dir = tempfile::tempdir().unwrap();

        // Old spec file — mtime far in the past.
        let old = dir.path().join("gw-spawn-old.json");
        std::fs::write(&old, "{}").unwrap();
        let past = SystemTime::now() - Duration::from_secs(48 * 3600);
        filetime::set_file_mtime(&old, filetime::FileTime::from_system_time(past)).unwrap();

        // Recent spec file — should survive.
        let recent = dir.path().join("gw-spawn-recent.json");
        std::fs::write(&recent, "{}").unwrap();

        // Unrelated file — should survive regardless of age.
        let unrelated = dir.path().join("something-else.json");
        std::fs::write(&unrelated, "{}").unwrap();
        filetime::set_file_mtime(&unrelated, filetime::FileTime::from_system_time(past)).unwrap();

        sweep_stale_in(dir.path(), Duration::from_secs(24 * 3600));

        assert!(!old.exists(), "old gw-spawn file should be removed");
        assert!(recent.exists(), "recent gw-spawn file should remain");
        assert!(unrelated.exists(), "unrelated file should be untouched");
    }
```

- [ ] **Step 2: Add `filetime` as a dev-dependency**

In `Cargo.toml` under `[dev-dependencies]`:

```toml
filetime = "0.2"
```

- [ ] **Step 3: Run — expect failure**

Run: `cargo test --lib spawn_spec`
Expected: FAIL — `sweep_stale_in` not found.

- [ ] **Step 4: Implement sweep**

Add to `src/operations/spawn_spec.rs`:

```rust
use std::time::{Duration, SystemTime};

/// Best-effort removal of stale `gw-spawn-*.json` temp files from the system
/// temp directory. Intended to run once at `gw` startup. All errors are
/// swallowed — this is a safety net, not a correctness mechanism.
pub fn sweep_stale() {
    sweep_stale_in(&std::env::temp_dir(), Duration::from_secs(24 * 3600));
}

fn sweep_stale_in(dir: &Path, max_age: Duration) {
    let entries = match fs::read_dir(dir) {
        Ok(it) => it,
        Err(_) => return,
    };
    let now = SystemTime::now();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if !name_str.starts_with("gw-spawn-") || !name_str.ends_with(".json") {
            continue;
        }
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let mtime = match metadata.modified() {
            Ok(t) => t,
            Err(_) => continue,
        };
        if now.duration_since(mtime).unwrap_or_default() > max_age {
            let _ = fs::remove_file(entry.path());
        }
    }
}
```

- [ ] **Step 5: Run tests — expect pass**

Run: `cargo test --lib spawn_spec`
Expected: 9 passed.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml src/operations/spawn_spec.rs
git commit -m "feat(spawn_spec): add 24h sweep_stale for crash-residue cleanup"
```

---

## Task 5: Wire `_spawn-ai` subcommand and replace `shell_quote_join`

**Files:**
- Modify: `src/cli.rs`
- Modify: `src/entrypoint.rs`
- Modify: `src/operations/ai_tools.rs`

- [ ] **Step 1: Add hidden `_spawn-ai` CLI variant**

In `src/cli.rs`, locate the `Commands` enum and add (alphabetically, next to the other `_*` hidden commands — after `HookEvents`):

```rust
    /// [Internal] Execute an AI tool spawn spec file
    #[command(name = "_spawn-ai", hide = true)]
    SpawnAi {
        /// Path to the JSON spawn spec
        #[arg(value_hint = ValueHint::FilePath)]
        spec: PathBuf,
    },
```

- [ ] **Step 2: Dispatch it in entrypoint**

In `src/entrypoint.rs`:

1. Add `spawn_spec` to the `use crate::operations::{...}` line (after `setup_claude,` alphabetically — between `shell` and `stash` let's place it next to related ops; the existing list isn't strictly alphabetical, so place `spawn_spec` adjacent to `shell`).

2. Add the `_spawn-ai` branch to the `is_internal` match (so startup update-checks are skipped — `_spawn-ai` runs on the AI-tool-launch hot path):

```rust
    let is_internal = matches!(
        &cli.command,
        Some(
            Commands::UpdateCache
                | Commands::ConfigKeys
                | Commands::TermValues
                | Commands::PresetNames
                | Commands::HookEvents
                | Commands::SpawnAi { .. }
        )
    );
```

3. Add a sweep call immediately after the `is_internal` decision but BEFORE `update::check_for_update_if_needed()`. The sweep runs on every non-internal invocation so leaked spec files are collected during ordinary `gw` usage:

```rust
    if !is_internal {
        crate::operations::spawn_spec::sweep_stale();
        update::check_for_update_if_needed();
    }
```

(Replace the existing `if !is_internal { update::check_for_update_if_needed(); }` block with the above.)

4. Add the dispatch arm inside the `match &cli.command` near the other `_*` hidden commands (for example after `Commands::HookEvents`):

```rust
        Some(Commands::SpawnAi { spec }) => spawn_spec::execute(spec),
```

- [ ] **Step 3: Run `cargo check` — expect clean build**

Run: `cargo check`
Expected: compiles.

- [ ] **Step 4: Replace `shell_quote_join` in `ai_tools.rs`**

In `src/operations/ai_tools.rs`:

Replace the `// Build shell command string` block (around line 65) and the `shell_quote_join` function (lines 278-291) with `spawn_spec::materialize`.

Add import near the top, inside the `use super::` block:

```rust
use super::spawn_spec::{self, SpawnSpec};
```

Replace:

```rust
    // Build shell command string
    let cmd = shell_quote_join(&ai_cmd_parts);
```

with:

```rust
    // Build a shell-safe wrapper line: the launcher shell only parses
    // `exec gw _spawn-ai <path>`; the raw argv (including user prompt) is in
    // a 0600 temp file that `_spawn-ai` reads and execvp's.
    let spec = SpawnSpec::new(ai_cmd_parts, path.to_path_buf());
    let (cmd, _spec_path) = spawn_spec::materialize(&spec)?;
```

Delete the entire `fn shell_quote_join` definition at the bottom of the file.

- [ ] **Step 5: Run `cargo build` — expect clean**

Run: `cargo build`
Expected: compiles with no warnings about unused imports.

- [ ] **Step 6: Run the full test suite — everything still green**

Run: `cargo test`
Expected: all existing tests pass; 9+ new `spawn_spec` tests pass.

- [ ] **Step 7: Clippy clean**

Run: `cargo clippy --all-targets -- -D warnings`
Expected: success.

- [ ] **Step 8: Commit**

```bash
git add src/cli.rs src/entrypoint.rs src/operations/ai_tools.rs
git commit -m "fix(ai-tools): route AI spawn through gw _spawn-ai wrapper

Drops shell_quote_join in favor of a tempfile spec read by a hidden
_spawn-ai subcommand. Eliminates shell-escape failures with quotes,
\$, backticks, backslashes, and multi-line content across every
launcher (foreground, detached, iterm, tmux, wezterm, zellij)."
```

---

## Task 6: Integration test — end-to-end prompt byte-exactness

**Files:**
- Create: `tests/spawn_roundtrip.rs`

This test invokes the compiled `gw` binary with `_spawn-ai`, pointing at a spec whose `argv[0]` is a shell helper that prints the received arg to stdout. We assert byte-exact equality with the input prompt.

- [ ] **Step 1: Write the integration test**

Create `tests/spawn_roundtrip.rs`:

```rust
//! End-to-end: `gw _spawn-ai <spec>` reads a spec and execvp's argv[0] with
//! argv[1..] verbatim. We point argv[0] at a platform-appropriate echo helper
//! and assert byte-for-byte prompt preservation.

use std::path::PathBuf;
use std::process::Command;

use assert_cmd::prelude::*;
use git_worktree_manager::operations::spawn_spec::SpawnSpec;
use tempfile::TempDir;

fn write_spec(dir: &TempDir, argv: Vec<String>, cwd: PathBuf) -> PathBuf {
    let spec = SpawnSpec::new(argv, cwd);
    let path = dir.path().join("spec.json");
    std::fs::write(&path, serde_json::to_vec(&spec).unwrap()).unwrap();
    path
}

#[cfg(unix)]
fn echo_program() -> String {
    "/bin/echo".into()
}

#[cfg(windows)]
fn echo_program() -> String {
    // cmd's builtin echo requires a shell; use PowerShell's Write-Output as a
    // standalone binary if present, otherwise fall back to `cmd /c echo`.
    "cmd".into()
}

#[cfg(unix)]
fn build_echo_argv(prompt: &str) -> Vec<String> {
    vec![echo_program(), "-n".into(), prompt.into()]
}

#[cfg(windows)]
fn build_echo_argv(prompt: &str) -> Vec<String> {
    vec![echo_program(), "/c".into(), "echo".into(), prompt.into()]
}

fn killer_prompts() -> Vec<&'static str> {
    vec![
        r#"Fix the bug where user can "escape" quotes"#,
        r#"$(rm -rf /) — literal, not an expansion"#,
        "한글 테스트 🚀 ${PATH}",
        "multi\nline\n<<'EOF'\nnot a heredoc\nEOF\n",
        r"C:\Users\foo\bar \\path\\with\\backslashes",
        "`backtick` and 'single' and \"double\"",
    ]
}

#[test]
fn spawn_ai_preserves_prompt_bytes_exactly() {
    for prompt in killer_prompts() {
        let dir = tempfile::tempdir().unwrap();
        let argv = build_echo_argv(prompt);
        let spec_path = write_spec(&dir, argv, dir.path().to_path_buf());

        let output = Command::cargo_bin("gw")
            .unwrap()
            .arg("_spawn-ai")
            .arg(&spec_path)
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "gw _spawn-ai failed: stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );

        let stdout = String::from_utf8_lossy(&output.stdout);
        #[cfg(unix)]
        let expected = prompt.to_string();
        #[cfg(windows)]
        let expected = format!("{}\r\n", prompt); // cmd echo adds CRLF

        assert_eq!(stdout, expected, "mismatch for prompt: {:?}", prompt);

        assert!(!spec_path.exists(), "spec file should be unlinked");
    }
}
```

- [ ] **Step 2: Run — expect pass**

Run: `cargo test --test spawn_roundtrip`
Expected: 1 passed.

If Windows CI is unavailable locally, mark the Windows arm with `#[ignore]` or rely on CI to verify. The primary target is Unix (issue report was w-t-b on macOS).

- [ ] **Step 3: Run the full suite**

Run: `cargo test`
Expected: all passing, including the new integration test.

- [ ] **Step 4: Commit**

```bash
git add tests/spawn_roundtrip.rs
git commit -m "test(spawn_spec): integration test for byte-exact prompt preservation"
```

---

## Task 7: Soften `--prompt-file` recommendation in setup-claude docs

**Files:**
- Modify: `src/operations/setup_claude.rs`

- [ ] **Step 1: Read the current wording**

Run: `grep -n "escap\|prompt-file\|Prefer" src/operations/setup_claude.rs`

Expected lines include the "avoids all shell escaping issues" wording and the "⭐ Recommended" marker around `--prompt-file`.

- [ ] **Step 2: Update wording**

In `src/operations/setup_claude.rs`:

- Change any phrasing implying `--prompt-file` is required to avoid escape bugs into "any of `--prompt`, `--prompt-file`, `--prompt-stdin` are equally safe; `--prompt-file` is convenient for editor-managed multi-line content."
- Leave the ⭐ marker on `--prompt-file` only if you keep it as "recommended for multi-line / editor-authored content," not "recommended because of escaping."
- Do NOT change CLI flags, examples' flag usage, or the overall structure of the doc — only the "why use this flag" prose.

Minimum required edit: replace any sentence claiming `--prompt` is unsafe for quoted/special content with one that says all three ingestion modes are safe.

- [ ] **Step 3: Run `cargo build` — expect success (no code path affected, but the file is included in the binary-embedded doc)**

Run: `cargo build`
Expected: success.

- [ ] **Step 4: Run `cargo test`**

Run: `cargo test`
Expected: all green.

- [ ] **Step 5: Commit**

```bash
git add src/operations/setup_claude.rs
git commit -m "docs(setup-claude): all prompt ingestion modes are escape-safe"
```

---

## Task 8: Manual verification and PR

**Files:** none (verification only).

- [ ] **Step 1: Build release binary**

Run: `cargo build --release`
Expected: `target/release/gw` present.

- [ ] **Step 2: Smoke each launcher with a killer prompt**

For each of the launchers below, run the command and verify the pane launches the AI tool with the prompt byte-exact (copy-paste the displayed prompt back into `diff` against the source if needed). Use a throwaway branch each time and `gw rm` afterwards.

Killer prompt file contents (save as `/tmp/gw-killer.txt`):

```text
Fix the bug where user can "escape" quotes — $(rm -rf /) should be literal.
한글 테스트 🚀 ${PATH}
Multi-line with backslashes: C:\Users\foo\bar
```

Commands:

```bash
gw new throwaway-fg    -T fg     --prompt-file /tmp/gw-killer.txt
gw new throwaway-det   -T detach --prompt-file /tmp/gw-killer.txt
gw new throwaway-it    -T i-t    --prompt-file /tmp/gw-killer.txt   # macOS
gw new throwaway-wez-b -T w-t-b  --prompt-file /tmp/gw-killer.txt   # primary gate
gw new throwaway-tmux  -T t-w    --prompt-file /tmp/gw-killer.txt   # if tmux running
gw new throwaway-zel   -T z-p    --prompt-file /tmp/gw-killer.txt   # if zellij running
```

For each: verify Claude's initial message matches the file verbatim. For `w-t-b`, additionally verify the originating tab retained focus.

- [ ] **Step 3: Clean up**

Run: `gw clean` (or `gw rm <branch>` for each).

- [ ] **Step 4: Open PR**

Run:

```bash
gh pr create --title "fix(ai-tools): eliminate shell-escape failures via gw _spawn-ai wrapper" --body "$(cat <<'EOF'
## Summary
- Launcher pipeline no longer shell-quotes user prompts. Instead, `ai_tools::launch_ai_tool` serializes argv + cwd into a 0600 tempfile and emits `exec gw _spawn-ai <path>`, which every launcher hands to its shell/pane unchanged.
- New hidden subcommand `gw _spawn-ai <spec>` reads the spec, unlinks it, `chdir`s, and `execvp`s the AI tool — the pane shell only ever parses ASCII.
- 24h best-effort sweep of stale `gw-spawn-*.json` runs at `gw` startup.
- Addresses recurring failures on `w-t-b` (and other launchers) with prompts containing quotes, `$`, backticks, backslashes, newlines, or non-ASCII.

Spec: `docs/superpowers/specs/2026-04-22-safe-ai-tool-spawn-design.md`
Plan: `docs/superpowers/plans/2026-04-22-safe-ai-tool-spawn.md`

## Test plan
- [x] `cargo test` — unit + integration (`spawn_roundtrip`) green
- [x] `cargo clippy --all-targets -- -D warnings`
- [x] Manual w-t-b with killer prompt preserves bytes exactly
- [x] Manual fg / detach / iterm-tab / tmux-window / zellij-pane / wezterm-window smoke
- [x] Stale sweep: leave a `gw-spawn-old.json` with 48h-old mtime, run any `gw` command, confirm it disappears
EOF
)"
```

Expected: PR URL returned.

---

## Self-Review Notes

**Spec coverage:**
- "Zero shell escape surface across launchers" → Task 5 (materialize replacement) + Task 6 (integration test).
- "Preserve interactive sessions" → Task 3 uses `execvp` (Unix) / child-spawn (Windows); no mode downgrade.
- "Uniform across all presets" → `ai_tools::launch_ai_tool` is the single entry point; replacement applies regardless of preset.
- "Leave `git_ops::create_pr` unchanged" → explicitly not modified; no Task touches it.
- Spec format, cleanup layers, error handling, Windows notes, testing strategy, rollout — all mapped to Tasks 1–8.

**Placeholder scan:** No TBDs, no "implement later," no vague "add error handling." Every code step contains the actual code. Task 7's minimum edit is a prose change and the instruction is explicit.

**Type consistency:**
- `SpawnSpec::new(argv, cwd)` defined in Task 1; used identically in Tasks 2, 3, 5, 6.
- `materialize` / `materialize_in_dir` / `read_spec` / `execute` / `sweep_stale` / `sweep_stale_in` — names stable across tasks.
- `Commands::SpawnAi { spec: PathBuf }` in Task 5 matches `spawn_spec::execute(spec: &Path)` (Rust coerces `&PathBuf` → `&Path` via deref).
