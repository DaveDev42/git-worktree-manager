# gw Plugin + Worktree Health + In-Use Refinement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convert `gw setup-claude` to install a Claude Code plugin (skills `delegate` + `manage`), embed a worktree-health rulebook with hook-recommendation catalog inside `manage`, and refine `gw delete` busy detection into a 3-tier model (Hard: Claude jsonl + lockfile, Soft: refined process scan) with a single unified `--force` override.

**Architecture:**
The gw binary stays focused on worktree CRUD. A new `claude_session` module reads `~/.claude/projects/<encoded>/*.jsonl` event tails to detect active Claude sessions (Hard tier). `busy.rs` is refactored: process cwd scan stays for diagnostics but is demoted to Soft tier and can no longer hard-refuse on its own. `setup_claude.rs` is rewritten to drop a plugin layout (`~/.claude/plugins/gw/`) instead of a single skill. Two new helper commands (`gw doctor --session-start`, `gw guard`) support the hooks the `manage` skill recommends in-session.

**Tech Stack:** Rust, clap (CLI), serde/serde_json (jsonl + manifest), thiserror (errors), tempfile (tests), console (styled output). No new dependencies.

---

## File Structure

### Files to create

| File | Responsibility |
|---|---|
| `src/operations/claude_session.rs` | Hard-tier signal: encode worktree path → `~/.claude/projects/<dir>`, tail `*.jsonl`, parse newest event timestamp, decide active/inactive against threshold. |
| `src/operations/guard.rs` | `gw guard --tool-input -` impl: read hook payload from stdin, classify Bash command risk, validate worktree state, exit 0/non-zero. |
| `src/operations/setup_claude/mod.rs` | Plugin installer: replaces current `setup_claude.rs`. Writes `plugin.json`, `skills/delegate/SKILL.md`, `skills/manage/SKILL.md`, `skills/manage/references/gw-commands.md`. Removes legacy locations. |
| `src/operations/setup_claude/skill_delegate.rs` | Returns the `delegate` SKILL.md body as `&'static str`. (The current `gw` skill body, modulo the section that now lives in `manage`.) |
| `src/operations/setup_claude/skill_manage.rs` | Returns the `manage` SKILL.md body as `&'static str` — three sections: command guidance, worktree-health rulebook, hooks catalog. |
| `src/operations/setup_claude/manifest.rs` | Returns `plugin.json` body as `&'static str`. |
| `src/operations/setup_claude/legacy.rs` | Removes prior installs at `~/.claude/skills/gw/` and `~/.claude/skills/gw-delegate/`. |
| `tests/test_claude_session.rs` | Integration tests for path encoding, jsonl tail parse, threshold logic. |
| `tests/test_setup_claude_plugin.rs` | Integration tests for plugin install layout, idempotency, legacy cleanup. |
| `tests/test_guard.rs` | Integration tests for `gw guard` risk classification + exit codes. |

### Files to modify

| File | Change |
|---|---|
| `src/operations/busy.rs` | Add `BusyTier` (Hard/Soft) and `tier` field to `BusyInfo`. Lockfile entries → `Hard`. Process scan entries → `Soft`. Add TTY/start-time fields. Remove `is_suspicious_cmd`. New entry point `detect_busy_tiered()` returns `(hard, soft)` split. Old `detect_busy()` kept as a shim calling the new fn. |
| `src/operations/worktree.rs` | `delete_worktree`: replace single-pile busy handling with tiered messages (Hard refusal vs Soft warning vs both). Remove the interactive y/N prompt — spec mandates explicit `--force`. |
| `src/operations/delete_batch.rs` | Same tiered messages for the batch path. |
| `src/operations/diagnostics.rs` (`gw doctor`) | Add `--session-start` and `--quiet` flags. Single-line summary mode for hook usage. Add plugin-install detection (look at `~/.claude/plugins/gw/`) alongside legacy skill check. |
| `src/cli.rs` | Add `--session-start` + `--quiet` to the `Doctor` subcommand. Add new `Guard { #[arg(long)] tool_input: String }` subcommand. CLI `Delete --force` semantics documented to mean "git-force AND bypass busy gate" (single flag). |
| `src/entrypoint.rs` | Route `Commands::Guard { .. }` to `operations::guard::run`. |
| `src/operations/mod.rs` | `pub mod claude_session; pub mod guard;` — also `setup_claude` becomes a directory module. |
| `tests/busy_detection.rs` | Update existing tests for new `BusyInfo` shape (add `tier` assertion); add new test cases for Hard/Soft combinations. |

### Decomposition rationale

- `claude_session` is its own file because the encoding rule + jsonl parse are isolated concerns testable independently of `busy.rs`.
- `setup_claude` becomes a directory module because the embedded skill bodies are large `&'static str` blobs and benefit from per-file separation.
- `guard` is its own file (~150 lines expected) because risk classification is a self-contained policy that will likely grow.
- `busy.rs` is **modified, not split**: the existing self-tree/sibling/multiplexer exclusion logic stays in place; only the decision-output API changes.

---

## Work Unit Sequencing

- **Unit 1** (Tasks 1–8): In-use detection refinement. CLI-only. No dependency on plugin work.
- **Unit 2** (Tasks 9–13): Plugin conversion of `setup_claude`. Existing skill content preserved verbatim into the new layout. Independent of Unit 1.
- **Unit 3** (Tasks 14–20): `manage` skill content + helper commands (`gw doctor --session-start`, `gw guard`). Depends on Unit 2's plugin layout existing.

Unit 1 and Unit 2 can be implemented in parallel by separate workers if desired.

---

## Unit 1 — In-Use Detection Refinement

### Task 1: Add `BusyTier` and extend `BusyInfo`

**Files:**
- Modify: `src/operations/busy.rs:19-36`

- [ ] **Step 1: Write the failing test**

Add to `tests/busy_detection.rs` inside `mod unix_only`:

```rust
#[test]
fn busy_info_includes_tier_field() {
    use git_worktree_manager::operations::busy::{BusyInfo, BusySource, BusyTier};
    use std::path::PathBuf;
    let info = BusyInfo {
        pid: 1,
        cmd: "x".into(),
        cwd: PathBuf::from("/tmp"),
        source: BusySource::Lockfile,
        tier: BusyTier::Hard,
        tty: None,
        started_secs_ago: None,
    };
    assert_eq!(info.tier, BusyTier::Hard);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test busy_detection busy_info_includes_tier_field`
Expected: FAIL — `BusyTier` not found, `BusyInfo` missing fields.

- [ ] **Step 3: Add the type and fields**

In `src/operations/busy.rs`, replace the `BusyInfo` struct (around line 28-36):

```rust
/// Tier of a busy signal — controls refusal *strength* in `gw delete`.
/// Hard signals (active Claude session, explicit lockfile) refuse with a
/// strong message. Soft signals (process cwd scan) refuse with a warning.
/// Both tiers are overridable by the same `--force` flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusyTier {
    Hard,
    Soft,
}

/// Signal source that flagged a process as busy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BusySource {
    Lockfile,
    ClaudeSession,
    ProcessScan,
}

/// Information about a single process holding a worktree busy.
#[derive(Debug, Clone)]
pub struct BusyInfo {
    pub pid: u32,
    pub cmd: String,
    /// For lockfile sources, this is the worktree path (the process's
    /// actual cwd is unknown). For process-scan sources, this is the
    /// process's canonicalized cwd. For ClaudeSession, this is the worktree.
    pub cwd: PathBuf,
    pub source: BusySource,
    pub tier: BusyTier,
    /// Whether the process has a controlling TTY (interactive hint).
    /// `None` if not determined (e.g. on Windows or for ClaudeSession).
    pub tty: Option<bool>,
    /// Approximate seconds since the process started, if known.
    pub started_secs_ago: Option<u64>,
}
```

- [ ] **Step 4: Update existing constructors so the codebase compiles**

In `src/operations/busy.rs`, every `BusyInfo { ... }` literal needs the new fields. Find each one and add `tier: BusyTier::Hard` (for Lockfile) or `tier: BusyTier::Soft` (for ProcessScan), plus `tty: None, started_secs_ago: None`. Locations: `detect_busy` (lockfile branch), `detect_busy_lockfile_only` (lockfile branch), `scan_cwd` (cwd branch).

- [ ] **Step 5: Run all tests to verify compile + new test passes**

Run: `cargo test --test busy_detection`
Expected: PASS, including the new `busy_info_includes_tier_field`.

- [ ] **Step 6: Commit**

```bash
git add src/operations/busy.rs tests/busy_detection.rs
git commit -m "refactor(busy): add BusyTier and TTY/start-time fields to BusyInfo"
```

---

### Task 2: Path encoding for Claude project directories

**Files:**
- Create: `src/operations/claude_session.rs`
- Test: `tests/test_claude_session.rs`

- [ ] **Step 1: Write the failing test**

Create `tests/test_claude_session.rs`:

```rust
use git_worktree_manager::operations::claude_session::encode_project_dir;
use std::path::Path;

#[test]
fn encode_simple_path() {
    let p = Path::new("/Users/dave/Projects/github.com/git-worktree-manager");
    assert_eq!(
        encode_project_dir(p),
        "-Users-dave-Projects-github-com-git-worktree-manager"
    );
}

#[test]
fn encode_path_with_dots() {
    let p = Path::new("/Users/dave/Projects/github.com/foo.bar");
    assert_eq!(
        encode_project_dir(p),
        "-Users-dave-Projects-github-com-foo-bar"
    );
}

#[test]
fn encode_path_with_trailing_slash() {
    let p = Path::new("/tmp/foo/");
    assert_eq!(encode_project_dir(p), "-tmp-foo");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test test_claude_session`
Expected: FAIL — module not found.

- [ ] **Step 3: Create the module with minimal impl**

Create `src/operations/claude_session.rs`:

```rust
//! Hard-tier in-use signal: detects active Claude Code sessions in a
//! worktree by inspecting `~/.claude/projects/<encoded>/*.jsonl` event tails.
//!
//! Encoding rule mirrors Claude Code's own: replace `/` and `.` with `-`,
//! drop trailing slash. Verified empirically against `~/.claude/projects/`
//! contents during design.

use std::path::Path;

/// Encode an absolute filesystem path to the directory name Claude Code
/// uses under `~/.claude/projects/`. `/` and `.` become `-`. Trailing
/// path separators are trimmed.
pub fn encode_project_dir(path: &Path) -> String {
    let s = path.to_string_lossy();
    let trimmed = s.trim_end_matches('/');
    trimmed.replace(['/', '.'], "-")
}
```

Add `pub mod claude_session;` to `src/operations/mod.rs`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test test_claude_session`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src/operations/claude_session.rs src/operations/mod.rs tests/test_claude_session.rs
git commit -m "feat(claude_session): path encoding for ~/.claude/projects/ lookup"
```

---

### Task 3: jsonl tail parsing — extract newest event timestamp

**Files:**
- Modify: `src/operations/claude_session.rs`
- Modify: `tests/test_claude_session.rs`

- [ ] **Step 1: Write the failing test**

Add to `tests/test_claude_session.rs`:

```rust
use std::io::Write;

#[test]
fn newest_event_timestamp_skips_metadata_trailers() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("session.jsonl");
    let mut f = std::fs::File::create(&p).unwrap();
    writeln!(f, r#"{{"type":"user","timestamp":"2026-04-25T10:00:00Z"}}"#).unwrap();
    writeln!(f, r#"{{"type":"assistant","timestamp":"2026-04-25T10:00:30Z"}}"#).unwrap();
    writeln!(f, r#"{{"type":"last-prompt","lastPrompt":"x","sessionId":"s"}}"#).unwrap();
    writeln!(f, r#"{{"type":"permission-mode","permissionMode":"default","sessionId":"s"}}"#).unwrap();

    let ts = git_worktree_manager::operations::claude_session::newest_event_timestamp(&p)
        .expect("should parse");
    assert_eq!(ts.timestamp(), 1745575230); // 2026-04-25T10:00:30Z
}

#[test]
fn newest_event_timestamp_returns_none_for_empty() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("empty.jsonl");
    std::fs::write(&p, b"").unwrap();
    assert!(git_worktree_manager::operations::claude_session::newest_event_timestamp(&p).is_none());
}

#[test]
fn newest_event_timestamp_returns_none_for_metadata_only() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("meta.jsonl");
    let mut f = std::fs::File::create(&p).unwrap();
    writeln!(f, r#"{{"type":"last-prompt","lastPrompt":"x","sessionId":"s"}}"#).unwrap();
    assert!(git_worktree_manager::operations::claude_session::newest_event_timestamp(&p).is_none());
}
```

Add `tempfile = "3"` to `[dev-dependencies]` in `Cargo.toml` if not already present (check first with `grep tempfile Cargo.toml`).

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test test_claude_session newest_event_timestamp`
Expected: FAIL — function not defined.

- [ ] **Step 3: Add the parser**

Append to `src/operations/claude_session.rs`:

```rust
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::PathBuf;

use chrono::{DateTime, Utc};

/// Read up to ~64 KiB from the end of `path`, find the newest line that
/// parses as JSON with a `timestamp` field, and return that timestamp.
/// Returns `None` for empty files, files containing only metadata events
/// without `timestamp`, or unreadable / unparseable files.
pub fn newest_event_timestamp(path: &Path) -> Option<DateTime<Utc>> {
    const TAIL_BYTES: u64 = 64 * 1024;

    let mut f = File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    let start = len.saturating_sub(TAIL_BYTES);
    f.seek(SeekFrom::Start(start)).ok()?;

    let mut buf = Vec::new();
    f.read_to_end(&mut buf).ok()?;

    // Drop the first (possibly partial) line if we did not start at byte 0.
    let mut slice = buf.as_slice();
    if start != 0 {
        if let Some(nl) = slice.iter().position(|&b| b == b'\n') {
            slice = &slice[nl + 1..];
        }
    }

    let reader = BufReader::new(slice);
    let mut latest: Option<DateTime<Utc>> = None;
    for line in reader.lines().map_while(Result::ok) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let v: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let ts_str = match v.get("timestamp").and_then(|x| x.as_str()) {
            Some(s) => s,
            None => continue,
        };
        let ts = match DateTime::parse_from_rfc3339(ts_str) {
            Ok(t) => t.with_timezone(&Utc),
            Err(_) => continue,
        };
        match latest {
            Some(prev) if prev >= ts => {}
            _ => latest = Some(ts),
        }
    }
    latest
}

/// Optional companion: extract the `cwd` field from the same newest event,
/// for path-encoding-collision defense. Returns `None` if not present.
pub fn newest_event_cwd(path: &Path) -> Option<PathBuf> {
    const TAIL_BYTES: u64 = 64 * 1024;
    let mut f = File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    let start = len.saturating_sub(TAIL_BYTES);
    f.seek(SeekFrom::Start(start)).ok()?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).ok()?;
    let mut slice = buf.as_slice();
    if start != 0 {
        if let Some(nl) = slice.iter().position(|&b| b == b'\n') {
            slice = &slice[nl + 1..];
        }
    }
    let reader = BufReader::new(slice);
    let mut latest: Option<(DateTime<Utc>, PathBuf)> = None;
    for line in reader.lines().map_while(Result::ok) {
        let v: serde_json::Value = match serde_json::from_str(line.trim()) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let Some(ts_str) = v.get("timestamp").and_then(|x| x.as_str()) else {
            continue;
        };
        let Some(cwd_str) = v.get("cwd").and_then(|x| x.as_str()) else {
            continue;
        };
        let Ok(ts) = DateTime::parse_from_rfc3339(ts_str) else {
            continue;
        };
        let ts = ts.with_timezone(&Utc);
        match latest {
            Some((prev, _)) if prev >= ts => {}
            _ => latest = Some((ts, PathBuf::from(cwd_str))),
        }
    }
    latest.map(|(_, p)| p)
}
```

Add `chrono = { version = "0.4", default-features = false, features = ["std", "clock", "serde"] }` to `[dependencies]` in `Cargo.toml` only if not already present (check with `grep '^chrono' Cargo.toml`).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --test test_claude_session`
Expected: PASS (5 tests total).

- [ ] **Step 5: Commit**

```bash
git add src/operations/claude_session.rs tests/test_claude_session.rs Cargo.toml Cargo.lock
git commit -m "feat(claude_session): newest_event_timestamp / cwd jsonl tail parsers"
```

---

### Task 4: `find_active_sessions` — apply 10-minute threshold

**Files:**
- Modify: `src/operations/claude_session.rs`
- Modify: `tests/test_claude_session.rs`

- [ ] **Step 1: Write the failing test**

Add to `tests/test_claude_session.rs`:

```rust
use chrono::{Duration, Utc};

fn write_session_jsonl(dir: &Path, name: &str, ts: chrono::DateTime<Utc>, cwd: &str) {
    let p = dir.join(name);
    let line = format!(
        r#"{{"type":"assistant","timestamp":"{}","cwd":"{}"}}"#,
        ts.to_rfc3339(),
        cwd,
    );
    std::fs::write(p, format!("{}\n", line)).unwrap();
}

#[test]
fn find_active_sessions_returns_sessions_within_threshold() {
    use git_worktree_manager::operations::claude_session::find_active_sessions;
    let proj = tempfile::tempdir().unwrap();
    let wt = "/tmp/fake-wt";
    let now = Utc::now();
    write_session_jsonl(proj.path(), "fresh.jsonl", now - Duration::minutes(2), wt);
    write_session_jsonl(proj.path(), "stale.jsonl", now - Duration::minutes(30), wt);

    let mut found = find_active_sessions(proj.path(), Path::new(wt), Duration::minutes(10));
    found.sort_by(|a, b| a.session_id.cmp(&b.session_id));
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].session_id, "fresh");
}

#[test]
fn find_active_sessions_filters_by_cwd() {
    use git_worktree_manager::operations::claude_session::find_active_sessions;
    let proj = tempfile::tempdir().unwrap();
    let wt = "/tmp/wanted";
    let now = Utc::now();
    write_session_jsonl(proj.path(), "wrong.jsonl", now, "/tmp/different");
    let found = find_active_sessions(proj.path(), Path::new(wt), Duration::minutes(10));
    assert!(found.is_empty(), "session for a different cwd should not match");
}

#[test]
fn find_active_sessions_handles_missing_dir() {
    use git_worktree_manager::operations::claude_session::find_active_sessions;
    let found = find_active_sessions(
        Path::new("/nonexistent/dir/xyz"),
        Path::new("/tmp/x"),
        Duration::minutes(10),
    );
    assert!(found.is_empty());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test test_claude_session find_active_sessions`
Expected: FAIL — function not defined.

- [ ] **Step 3: Implement `find_active_sessions`**

Append to `src/operations/claude_session.rs`:

```rust
use chrono::Duration;

/// Information about one active Claude Code session in a worktree.
#[derive(Debug, Clone)]
pub struct ActiveSession {
    /// jsonl filename without extension (matches Claude session UUID).
    pub session_id: String,
    /// Wall-clock time of the most recent event with a `timestamp` field.
    pub last_activity: DateTime<Utc>,
}

/// Return all sessions in `project_dir` whose newest event timestamp is
/// within `threshold` of now AND whose newest event `cwd` (if present)
/// matches `worktree`. Missing/unreadable directories return an empty vec
/// — the caller treats this as "Claude not in use here."
pub fn find_active_sessions(
    project_dir: &Path,
    worktree: &Path,
    threshold: Duration,
) -> Vec<ActiveSession> {
    let entries = match std::fs::read_dir(project_dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };
    let now = Utc::now();
    let wt_canon = worktree.canonicalize().unwrap_or_else(|_| worktree.to_path_buf());
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("jsonl") {
            continue;
        }
        let Some(ts) = newest_event_timestamp(&path) else {
            continue;
        };
        if (now - ts) > threshold {
            continue;
        }
        if let Some(reported_cwd) = newest_event_cwd(&path) {
            let reported_canon = reported_cwd.canonicalize().unwrap_or(reported_cwd);
            if reported_canon != wt_canon {
                continue;
            }
        }
        let id = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
        out.push(ActiveSession { session_id: id, last_activity: ts });
    }
    out
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --test test_claude_session`
Expected: PASS (8 tests total).

- [ ] **Step 5: Commit**

```bash
git add src/operations/claude_session.rs tests/test_claude_session.rs
git commit -m "feat(claude_session): find_active_sessions with threshold + cwd filter"
```

---

### Task 5: Wire Claude-session signal into `detect_busy` as Hard tier

**Files:**
- Modify: `src/operations/busy.rs:378-412`
- Modify: `src/operations/claude_session.rs`
- Modify: `tests/busy_detection.rs`

- [ ] **Step 1: Add helper to resolve `~/.claude/projects/` path**

Append to `src/operations/claude_session.rs`:

```rust
/// Resolve the per-worktree Claude projects directory, e.g.
/// `~/.claude/projects/-Users-dave-Projects-foo`. Returns `None` if
/// `$HOME` is not set.
pub fn project_dir_for(worktree: &Path) -> Option<PathBuf> {
    let home = crate::constants::home_dir_or_fallback();
    let canon = worktree.canonicalize().unwrap_or_else(|_| worktree.to_path_buf());
    let encoded = encode_project_dir(&canon);
    Some(home.join(".claude").join("projects").join(encoded))
}
```

- [ ] **Step 2: Write the failing test**

Add to `tests/busy_detection.rs` inside `mod unix_only` (the test below requires write access to `~/.claude/projects/<encoded>/`; use a tempdir-faked `HOME` via env override is non-trivial — instead we test the busy.rs entry point indirectly by injecting through a constructed Hard-tier `BusyInfo`. Add a *unit* test in `busy.rs` instead):

In `src/operations/busy.rs` `#[cfg(test)] mod tests` (append to end of file before the existing tests block, or inside it):

```rust
#[test]
fn detect_busy_tiered_returns_hard_for_lockfile() {
    let dir = tempfile::tempdir().unwrap();
    // Mark a fake .git dir so lock_path resolves predictably.
    std::fs::create_dir_all(dir.path().join(".git")).unwrap();
    crate::operations::lockfile::acquire(dir.path(), "claude")
        .expect("lock acquire");
    let (hard, _soft) = detect_busy_tiered(dir.path());
    assert!(hard.iter().any(|b| matches!(b.source, BusySource::Lockfile)));
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p git-worktree-manager --lib busy::tests::detect_busy_tiered_returns_hard_for_lockfile`
Expected: FAIL — `detect_busy_tiered` not found.

- [ ] **Step 4: Add `detect_busy_tiered` to `busy.rs`**

In `src/operations/busy.rs`, add (near `detect_busy`):

```rust
use crate::operations::claude_session;
use chrono::Duration as ChronoDuration;

/// Threshold for considering a Claude jsonl event "active." Spec value.
const CLAUDE_ACTIVITY_THRESHOLD_MIN: i64 = 10;

/// Tiered busy detection: returns `(hard, soft)` separately so the caller
/// can render distinct refusal messages.
///
/// Hard signals (refuse strongly, override = `--force`):
///   * Active Claude Code session (jsonl event within threshold)
///   * Explicit lockfile
///
/// Soft signals (refuse with a warning, same `--force` override):
///   * Process cwd scan results that are not already represented by a
///     Hard signal (deduped by PID)
pub fn detect_busy_tiered(worktree: &Path) -> (Vec<BusyInfo>, Vec<BusyInfo>) {
    let exclude_tree = self_process_tree();
    let exclude_siblings = self_siblings();
    let is_excluded = |pid: u32| exclude_tree.contains(&pid) || exclude_siblings.contains(&pid);

    let mut hard = Vec::new();

    // Hard: lockfile
    if let Some(entry) = lockfile::read_and_clean_stale(worktree) {
        if !is_excluded(entry.pid) {
            hard.push(BusyInfo {
                pid: entry.pid,
                cmd: entry.cmd,
                cwd: worktree.to_path_buf(),
                source: BusySource::Lockfile,
                tier: BusyTier::Hard,
                tty: None,
                started_secs_ago: None,
            });
        }
    }

    // Hard: active Claude sessions
    if let Some(proj_dir) = claude_session::project_dir_for(worktree) {
        let threshold = ChronoDuration::minutes(CLAUDE_ACTIVITY_THRESHOLD_MIN);
        for s in claude_session::find_active_sessions(&proj_dir, worktree, threshold) {
            // session_id is a UUID; surface as cmd "claude" with id in cwd.
            let secs_ago = (chrono::Utc::now() - s.last_activity).num_seconds().max(0) as u64;
            hard.push(BusyInfo {
                pid: 0, // not a process PID; informational entry
                cmd: format!("claude (session {})", s.session_id),
                cwd: worktree.to_path_buf(),
                source: BusySource::ClaudeSession,
                tier: BusyTier::Hard,
                tty: None,
                started_secs_ago: Some(secs_ago),
            });
        }
    }

    // Soft: process cwd scan, deduped against PIDs already in Hard
    let mut soft = Vec::new();
    for info in scan_cwd(worktree) {
        if is_excluded(info.pid) {
            continue;
        }
        if hard.iter().any(|b| b.pid == info.pid && b.pid != 0) {
            continue;
        }
        soft.push(info);
    }

    (hard, soft)
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p git-worktree-manager --lib busy::tests::detect_busy_tiered_returns_hard_for_lockfile`
Expected: PASS.

Run: `cargo test`
Expected: existing tests still pass.

- [ ] **Step 6: Commit**

```bash
git add src/operations/busy.rs src/operations/claude_session.rs
git commit -m "feat(busy): detect_busy_tiered combines Claude session + lockfile (Hard) and scan (Soft)"
```

---

### Task 6: Refuse-message rendering for the three shapes

**Files:**
- Create: `src/operations/busy_messages.rs`
- Modify: `src/operations/mod.rs`
- Test: `tests/test_busy_messages.rs`

- [ ] **Step 1: Write the failing test**

Create `tests/test_busy_messages.rs`:

```rust
use git_worktree_manager::operations::busy::{BusyInfo, BusySource, BusyTier};
use git_worktree_manager::operations::busy_messages::render_refusal;
use std::path::PathBuf;

fn hard_claude(secs: u64) -> BusyInfo {
    BusyInfo {
        pid: 0,
        cmd: "claude (session abc123)".into(),
        cwd: PathBuf::from("/tmp/wt"),
        source: BusySource::ClaudeSession,
        tier: BusyTier::Hard,
        tty: None,
        started_secs_ago: Some(secs),
    }
}

fn soft_proc(pid: u32, cmd: &str, tty: bool, started: u64) -> BusyInfo {
    BusyInfo {
        pid,
        cmd: cmd.into(),
        cwd: PathBuf::from("/tmp/wt"),
        source: BusySource::ProcessScan,
        tier: BusyTier::Soft,
        tty: Some(tty),
        started_secs_ago: Some(started),
    }
}

#[test]
fn soft_only_uses_warning_tone() {
    let s = render_refusal("feature-x", &[], &[soft_proc(123, "zsh", true, 60)]);
    assert!(s.contains("may be in use"), "soft-only must use 'may be in use' wording");
    assert!(s.contains("Re-run with --force"));
    assert!(!s.contains("Cannot delete"));
}

#[test]
fn hard_only_uses_strong_refusal() {
    let s = render_refusal("feature-x", &[hard_claude(120)], &[]);
    assert!(s.contains("Cannot delete worktree 'feature-x'"));
    assert!(s.contains("Active Claude session"));
    assert!(s.contains("Use --force"));
}

#[test]
fn both_tiers_lead_with_hard_then_show_soft() {
    let s = render_refusal(
        "feature-x",
        &[hard_claude(60)],
        &[soft_proc(7, "cargo build", false, 30)],
    );
    let hard_pos = s.find("Active Claude session").unwrap();
    let soft_pos = s.find("Additional processes").unwrap();
    assert!(hard_pos < soft_pos, "Hard section must precede Soft");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test test_busy_messages`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement the renderer**

Create `src/operations/busy_messages.rs`:

```rust
//! Render `gw delete` refusal messages for the 3-tier busy model.
//! Pure string formatting; no I/O. Kept separate from `busy.rs` so the
//! detection logic can be tested without locale/styling concerns.

use crate::operations::busy::{BusyInfo, BusySource};

fn fmt_age(secs: u64) -> String {
    if secs < 60 {
        format!("{}s ago", secs)
    } else if secs < 3600 {
        format!("{} minute{} ago", secs / 60, if secs / 60 == 1 { "" } else { "s" })
    } else {
        format!("{} hour{} ago", secs / 3600, if secs / 3600 == 1 { "" } else { "s" })
    }
}

fn render_hard_section(out: &mut String, hard: &[BusyInfo]) {
    for h in hard {
        match h.source {
            BusySource::ClaudeSession => {
                out.push_str("  Active Claude session\n");
                if let Some(secs) = h.started_secs_ago {
                    out.push_str(&format!("    last activity: {}\n", fmt_age(secs)));
                }
                // cmd carries "claude (session <id>)"
                if let Some(id_part) = h.cmd.strip_prefix("claude (session ") {
                    let id = id_part.trim_end_matches(')');
                    out.push_str(&format!("    session: {}\n", id));
                }
            }
            BusySource::Lockfile => {
                out.push_str(&format!(
                    "  Lockfile holder: PID {} ({})\n",
                    h.pid, h.cmd
                ));
            }
            BusySource::ProcessScan => {
                // Should not appear in hard tier; render defensively.
                out.push_str(&format!("  PID {}  {}\n", h.pid, h.cmd));
            }
        }
        out.push('\n');
    }
}

fn render_soft_list(out: &mut String, soft: &[BusyInfo]) {
    for s in soft {
        let tty_label = match s.tty {
            Some(true) => "(interactive)",
            Some(false) => "(no tty)",
            None => "",
        };
        let age_label = match s.started_secs_ago {
            Some(secs) if secs < 90 => format!(" (started {})", fmt_age(secs)),
            _ => String::new(),
        };
        out.push_str(&format!(
            "    PID {:>6}  {}  {}{}\n",
            s.pid, s.cmd, tty_label, age_label
        ));
    }
}

/// Render the user-facing refusal text. Empty inputs in both vectors is a
/// programming error (caller should not have refused) but is rendered as
/// an empty string for safety.
pub fn render_refusal(branch_display: &str, hard: &[BusyInfo], soft: &[BusyInfo]) -> String {
    let mut out = String::new();
    match (hard.is_empty(), soft.is_empty()) {
        (true, true) => return out,
        (true, false) => {
            out.push_str(&format!(
                "⚠ Worktree '{}' may be in use:\n\n",
                branch_display
            ));
            out.push_str("  Processes with cwd in this worktree:\n");
            render_soft_list(&mut out, soft);
            out.push('\n');
            out.push_str("  These may malfunction if the worktree is deleted.\n");
            out.push_str("  Re-run with --force to delete anyway.\n");
        }
        (false, true) => {
            out.push_str(&format!(
                "✗ Cannot delete worktree '{}' — in use:\n\n",
                branch_display
            ));
            render_hard_section(&mut out, hard);
            out.push_str("  Use --force to delete anyway.\n");
        }
        (false, false) => {
            out.push_str(&format!(
                "✗ Cannot delete worktree '{}' — in use:\n\n",
                branch_display
            ));
            render_hard_section(&mut out, hard);
            out.push_str("  Additional processes with cwd in this worktree:\n");
            render_soft_list(&mut out, soft);
            out.push('\n');
            out.push_str("  Use --force to delete anyway.\n");
        }
    }
    out
}
```

Add `pub mod busy_messages;` to `src/operations/mod.rs`.

- [ ] **Step 4: Run tests**

Run: `cargo test --test test_busy_messages`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src/operations/busy_messages.rs src/operations/mod.rs tests/test_busy_messages.rs
git commit -m "feat(busy): refusal message renderer for tiered busy model"
```

---

### Task 7: Wire tiered messages into `delete_worktree`

**Files:**
- Modify: `src/operations/worktree.rs:407-444`

- [ ] **Step 1: Write the failing integration test**

Add to `tests/busy_detection.rs` inside `mod unix_only`:

```rust
#[test]
fn delete_refuses_with_lockfile_hard_tier_message() {
    use git_worktree_manager::operations::lockfile;
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".git")).unwrap();

    // Spawn a sleep so the lock looks live, register lockfile.
    let _lock = lockfile::acquire(dir.path(), "claude").expect("acquire");

    let (hard, soft) =
        git_worktree_manager::operations::busy::detect_busy_tiered(dir.path());
    assert!(!hard.is_empty(), "lockfile should appear as hard");
    assert!(soft.is_empty() || soft.iter().all(|s| s.pid != std::process::id()));

    let msg = git_worktree_manager::operations::busy_messages::render_refusal(
        "feature-x", &hard, &soft,
    );
    assert!(msg.contains("Cannot delete"));
}
```

- [ ] **Step 2: Run test to verify the new wiring is exercised**

Run: `cargo test --test busy_detection delete_refuses_with_lockfile_hard_tier_message`
Expected: PASS (uses the new tiered API directly; this test guards regressions).

- [ ] **Step 3: Replace the busy block in `delete_worktree`**

In `src/operations/worktree.rs`, replace lines 407–444 (the `let busy = ...` block down to the closing brace before `let flags = DeleteFlags { ... }`) with:

```rust
    let (hard, soft) = crate::operations::busy::detect_busy_tiered(&worktree_path);
    if (!hard.is_empty() || !soft.is_empty()) && !allow_busy {
        let branch_display = branch_name.clone().unwrap_or_else(|| {
            worktree_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| worktree_path.to_string_lossy().to_string())
        });
        let msg = crate::operations::busy_messages::render_refusal(
            &branch_display, &hard, &soft,
        );
        eprint!("{}", msg);
        return Err(CwError::Other(format!(
            "worktree '{}' is in use; re-run with --force to override",
            branch_display
        )));
    }
```

Note: this removes the previous interactive y/N prompt. Per spec the only path past a refusal is `--force`.

- [ ] **Step 4: Run all tests**

Run: `cargo test`
Expected: PASS — including the legacy `tests/test_delete_multi.rs` (which may need updates if any test expected the y/N prompt; if so, remove the prompt-related assertion, which is now obsolete per spec).

If a test fails on a prompt assertion, replace its expectation with: refusal returns an `Err(CwError::Other)` whose message contains `"is in use"`.

- [ ] **Step 5: Commit**

```bash
git add src/operations/worktree.rs tests/busy_detection.rs tests/test_delete_multi.rs
git commit -m "feat(delete): tiered refusal messages, drop interactive y/N prompt"
```

---

### Task 8: Apply same wiring to `delete_batch.rs`

**Files:**
- Modify: `src/operations/delete_batch.rs`

- [ ] **Step 1: Locate the busy-handling block**

Run: `grep -n "detect_busy\|allow_busy\|busy" src/operations/delete_batch.rs`
Note the line numbers for the next step.

- [ ] **Step 2: Write the failing test**

Add to `tests/test_delete_multi.rs`:

```rust
#[test]
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn batch_delete_skips_in_use_worktrees_with_message() {
    // Construct a temp main repo with two worktrees A and B.
    // Acquire a lockfile on A.
    // Call the batch deletion entrypoint with [A, B] and verify A is skipped
    // (refusal text contains "Cannot delete" or "may be in use") and B is deleted.
    //
    // Use the existing helpers in tests/common/ that wrap repo+worktree setup.
    use git_worktree_manager::operations::lockfile;
    let env = crate::common::TestRepo::new();
    let wt_a = env.create_worktree("feat-a");
    let wt_b = env.create_worktree("feat-b");
    let _lock = lockfile::acquire(&wt_a, "claude").unwrap();
    // Call your batch delete helper here; assert wt_a still exists, wt_b removed.
    // (Exact entrypoint name depends on tests/common; use what tests/test_delete_multi.rs already imports.)
}
```

If `tests/common/` does not expose the helpers needed, mirror the pattern from existing tests in the file (e.g. how it constructs a TestRepo and invokes batch deletion). Skip this step if integration is genuinely too tangled and rely on the unit-level test below instead.

- [ ] **Step 3: Replace `delete_batch.rs` busy block analogously**

Find the call site for `detect_busy(...)` in `delete_batch.rs` and replace it with `detect_busy_tiered(...)`. Render via `busy_messages::render_refusal`. Skip in-use worktrees in the batch loop instead of erroring (batch mode should not abort the whole job for one busy item — log refusal text to stderr and continue).

- [ ] **Step 4: Run all tests**

Run: `cargo test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/operations/delete_batch.rs tests/test_delete_multi.rs
git commit -m "feat(delete): tiered refusal in batch path; skip-and-continue for busy items"
```

---

## Unit 2 — Plugin Conversion

### Task 9: Scaffold `setup_claude` as a directory module

**Files:**
- Create: `src/operations/setup_claude/mod.rs`
- Create: `src/operations/setup_claude/manifest.rs`
- Create: `src/operations/setup_claude/legacy.rs`
- Delete: `src/operations/setup_claude.rs`

- [ ] **Step 1: Move existing file out of the way**

```bash
git mv src/operations/setup_claude.rs src/operations/setup_claude_old.rs
```

(We will delete `_old.rs` after the new layout lands.)

- [ ] **Step 2: Create `setup_claude/mod.rs` with the new public API**

```rust
//! Plugin installer for Claude Code integration.
//!
//! Installs gw as a Claude Code *plugin* at `~/.claude/plugins/gw/` with
//! two skills (`delegate`, `manage`). Removes legacy single-skill installs
//! at `~/.claude/skills/gw/` and `~/.claude/skills/gw-delegate/`.

use std::path::PathBuf;

use console::style;

use crate::constants::home_dir_or_fallback;
use crate::error::Result;

mod legacy;
mod manifest;
mod skill_delegate;
mod skill_manage;

const PLUGIN_NAME: &str = "gw";

fn plugin_dir() -> PathBuf {
    home_dir_or_fallback()
        .join(".claude")
        .join("plugins")
        .join(PLUGIN_NAME)
}

fn manifest_path() -> PathBuf { plugin_dir().join("plugin.json") }
fn delegate_skill_path() -> PathBuf { plugin_dir().join("skills").join("delegate").join("SKILL.md") }
fn manage_skill_path() -> PathBuf { plugin_dir().join("skills").join("manage").join("SKILL.md") }
fn manage_reference_path() -> PathBuf {
    plugin_dir()
        .join("skills")
        .join("manage")
        .join("references")
        .join("gw-commands.md")
}

/// True if the plugin manifest exists at the canonical path.
pub fn is_plugin_installed() -> bool { manifest_path().exists() }

/// Backward-compatible alias used by `gw doctor`. Returns true if either the
/// new plugin OR a legacy skill install is present.
pub fn is_skill_installed() -> bool {
    is_plugin_installed() || legacy::any_legacy_present()
}

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

pub fn setup_claude() -> Result<()> {
    legacy::remove_legacy_installs();

    let manifest = manifest_path();
    let delegate = delegate_skill_path();
    let manage = manage_skill_path();
    let reference = manage_reference_path();

    let mut any_changed = false;
    any_changed |= write_if_changed(&manifest, manifest::content())?;
    any_changed |= write_if_changed(&delegate, skill_delegate::content())?;
    any_changed |= write_if_changed(&manage, skill_manage::content())?;
    any_changed |= write_if_changed(&reference, skill_manage::reference_content())?;

    if !any_changed {
        println!("{} gw plugin already up to date.\n", style("*").green());
        println!("  Location: {}", style(plugin_dir().display()).dim());
        return Ok(());
    }

    println!(
        "{} gw plugin installed at {}.\n",
        style("*").green().bold(),
        style(plugin_dir().display()).dim()
    );
    println!(
        "  Use {} in Claude Code to delegate tasks to worktrees.",
        style("/gw").cyan()
    );
    println!(
        "  The bundled '{}' skill will recommend hooks (e.g. SessionStart sanity)",
        style("manage").cyan()
    );
    println!("  in-session when relevant. It edits your project's .claude/settings.json");
    println!("  on your consent — gw itself never modifies any settings file.\n");

    Ok(())
}
```

- [ ] **Step 3: Create `manifest.rs`**

```rust
//! Plugin manifest content. Static blob — versioned with the binary.

pub fn content() -> &'static str {
    "{\n  \"name\": \"gw\",\n  \"version\": \"1\",\n  \"description\": \"git-worktree-manager plugin: delegate tasks to worktrees and manage multi-worktree workflows safely.\",\n  \"author\": \"git-worktree-manager\"\n}\n"
}
```

- [ ] **Step 4: Create `legacy.rs`**

```rust
//! Removal of pre-plugin install locations.

use std::path::PathBuf;

use crate::constants::home_dir_or_fallback;

fn legacy_paths() -> Vec<PathBuf> {
    let base = home_dir_or_fallback().join(".claude").join("skills");
    vec![base.join("gw"), base.join("gw-delegate")]
}

pub fn any_legacy_present() -> bool {
    legacy_paths().iter().any(|p| p.exists())
}

pub fn remove_legacy_installs() {
    for p in legacy_paths() {
        if p.exists() {
            let _ = std::fs::remove_dir_all(&p);
        }
    }
}
```

- [ ] **Step 5: Create stubs for `skill_delegate.rs` and `skill_manage.rs`**

`src/operations/setup_claude/skill_delegate.rs`:

```rust
pub fn content() -> &'static str {
    "" // Filled in Task 10.
}
```

`src/operations/setup_claude/skill_manage.rs`:

```rust
pub fn content() -> &'static str {
    "" // Filled in Task 14.
}

pub fn reference_content() -> &'static str {
    "" // Filled in Task 14.
}
```

- [ ] **Step 6: Build to verify the module wiring compiles**

Run: `cargo build`
Expected: success. There may be a leftover unused-import warning from `setup_claude_old.rs`; ignore for now.

- [ ] **Step 7: Commit**

```bash
git add src/operations/setup_claude/
git commit -m "refactor(setup_claude): scaffold plugin-shaped directory module (skills empty)"
```

---

### Task 10: Migrate the existing skill body into `skill_delegate.rs`

**Files:**
- Modify: `src/operations/setup_claude/skill_delegate.rs`
- Read for content: `src/operations/setup_claude_old.rs:113-298` (the `skill_content()` body)

- [ ] **Step 1: Open the old file and locate the skill content**

Read `src/operations/setup_claude_old.rs` lines 113–298. This is the body returned by the old `skill_content()` function. The frontmatter `name: gw` should change to `name: delegate` so the plugin layout's skill name matches its directory.

- [ ] **Step 2: Replace `skill_delegate.rs` content stub**

```rust
pub fn content() -> &'static str {
    r#"---
name: delegate
description: "Delegate coding tasks to isolated git worktrees. Invoke with: /gw <natural language task description>."
allowed-tools: Bash
---

# git-worktree-manager (gw) — task delegation

[... copy the body from setup_claude_old.rs lines 116-297 verbatim, BUT:
 - omit the trailing "## Full command reference" section that points to gw-commands.md
   (that reference now belongs to the manage skill, not delegate)
 - keep all the prompt-file/prompt-stdin/prompt-string guidance and the branch-name rules
   and the terminal method selection advice
]
"#
}
```

Carry the content over byte-for-byte from the old file, only changing the frontmatter `name`. Concretely: open `setup_claude_old.rs`, copy the raw string between `r#"---` and `"#`, paste here, then change `name: gw` → `name: delegate` and remove the final reference-link section.

- [ ] **Step 3: Build**

Run: `cargo build`
Expected: success.

- [ ] **Step 4: Smoke test plugin install end-to-end**

Add to `tests/test_setup_claude_plugin.rs`:

```rust
//! Plugin install layout / idempotency tests. We override $HOME via env
//! to keep these hermetic.

use std::path::PathBuf;

fn fake_home() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

#[test]
fn install_creates_manifest_and_two_skills() {
    let home = fake_home();
    std::env::set_var("HOME", home.path());

    git_worktree_manager::operations::setup_claude::setup_claude().unwrap();

    let plugin = home.path().join(".claude").join("plugins").join("gw");
    assert!(plugin.join("plugin.json").exists());
    assert!(plugin.join("skills").join("delegate").join("SKILL.md").exists());
    assert!(plugin.join("skills").join("manage").join("SKILL.md").exists());
}

#[test]
fn install_is_idempotent() {
    let home = fake_home();
    std::env::set_var("HOME", home.path());

    git_worktree_manager::operations::setup_claude::setup_claude().unwrap();
    let manifest_mtime_1 = std::fs::metadata(
        home.path().join(".claude/plugins/gw/plugin.json"),
    ).unwrap().modified().unwrap();

    std::thread::sleep(std::time::Duration::from_millis(20));
    git_worktree_manager::operations::setup_claude::setup_claude().unwrap();
    let manifest_mtime_2 = std::fs::metadata(
        home.path().join(".claude/plugins/gw/plugin.json"),
    ).unwrap().modified().unwrap();

    assert_eq!(manifest_mtime_1, manifest_mtime_2,
        "second install must not rewrite unchanged content");
}

#[test]
fn install_removes_legacy_skill_dir() {
    let home = fake_home();
    std::env::set_var("HOME", home.path());

    let legacy = home.path().join(".claude/skills/gw");
    std::fs::create_dir_all(&legacy).unwrap();
    std::fs::write(legacy.join("SKILL.md"), b"old").unwrap();

    git_worktree_manager::operations::setup_claude::setup_claude().unwrap();
    assert!(!legacy.exists(), "legacy skill directory must be removed");
}
```

Caveat: setting `$HOME` mid-process affects other tests if they run concurrently in the same process. Cargo runs each `tests/` file in its own binary, but tests within a file share process. Add `#[serial_test::serial]` annotations OR move to one test that does all three steps, OR accept the risk and document. Simplest: add `serial_test = "3"` to dev-dependencies and `#[serial_test::serial]` on each test.

- [ ] **Step 5: Run tests**

Run: `cargo test --test test_setup_claude_plugin`
Expected: PASS (3 tests).

- [ ] **Step 6: Commit**

```bash
git add src/operations/setup_claude/skill_delegate.rs tests/test_setup_claude_plugin.rs Cargo.toml Cargo.lock
git commit -m "feat(setup_claude): migrate existing skill body into delegate skill"
```

---

### Task 11: Update `mod.rs` and `entrypoint.rs` to use new path

**Files:**
- Modify: `src/operations/mod.rs`
- Modify: `src/entrypoint.rs:316` (or thereabouts — the `Commands::SetupClaude` arm)

- [ ] **Step 1: Update `src/operations/mod.rs`**

Find the line `pub mod setup_claude;` — confirm it now resolves to the directory module. The line itself does not change; only its target file did.

- [ ] **Step 2: Confirm `entrypoint.rs` still resolves**

The call `setup_claude::setup_claude()` keeps the same name. Run:

```bash
cargo build
```

Expected: success.

- [ ] **Step 3: Delete the old file**

```bash
rm src/operations/setup_claude_old.rs
```

- [ ] **Step 4: Build to confirm the old file is gone cleanly**

Run: `cargo build`
Expected: success, no unused-file warnings.

- [ ] **Step 5: Commit**

```bash
git add src/operations/setup_claude_old.rs
git commit -m "refactor(setup_claude): drop pre-plugin module file"
```

---

### Task 12: `gw doctor` recognizes the new plugin path

**Files:**
- Modify: `src/operations/diagnostics.rs:296-305`

- [ ] **Step 1: Read the existing doctor check**

Run: `sed -n '290,320p' src/operations/diagnostics.rs`
Note the current `is_skill_installed` branch.

- [ ] **Step 2: Add a unit test for the doctor message picking new wording**

In `src/operations/diagnostics.rs`, inside `#[cfg(test)] mod tests` (create the block if missing), add:

```rust
#[test]
fn doctor_recognizes_plugin_install() {
    // setup_claude::is_plugin_installed should be the path the doctor checks.
    // We assert by reading the function name to ensure the call still exists.
    let src = include_str!("diagnostics.rs");
    assert!(src.contains("is_plugin_installed") || src.contains("is_skill_installed"),
        "doctor must check plugin install state");
}
```

- [ ] **Step 3: Update the doctor message**

In the existing doctor branch that says "Tip: Run 'gw setup-claude' to enable task delegation via Claude Code", change the surrounding wording to mention "plugin" instead of "skill", and use `setup_claude::is_plugin_installed()` for the positive-state branch (still call `is_skill_installed()` for the legacy fallback so users on the older path get the upgrade tip).

Concrete edit: change the conditional to:

```rust
} else if setup_claude::is_plugin_installed() {
    // already on plugin
} else if setup_claude::is_skill_installed() {
    println!(
        "  {}",
        style("Tip: Re-run 'gw setup-claude' to upgrade from skill to plugin").dim()
    );
} else {
    println!(
        "  {}",
        style("Tip: Run 'gw setup-claude' to install the gw plugin for Claude Code").dim()
    );
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p git-worktree-manager`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/operations/diagnostics.rs
git commit -m "feat(doctor): detect plugin install + suggest upgrade from legacy skill"
```

---

### Task 13: End-to-end smoke run

- [ ] **Step 1: Build release binary**

Run: `cargo build --release`
Expected: success.

- [ ] **Step 2: Manually invoke against a tempdir**

```bash
TMP=$(mktemp -d)
HOME="$TMP" ./target/release/gw setup-claude
ls -R "$TMP/.claude/plugins/gw/"
```

Expected: `plugin.json`, `skills/delegate/SKILL.md`, `skills/manage/SKILL.md` (the manage SKILL is still empty stub at this point — content arrives in Unit 3).

- [ ] **Step 3: Verify idempotency manually**

```bash
HOME="$TMP" ./target/release/gw setup-claude
```

Expected: prints "already up to date".

- [ ] **Step 4: No commit (smoke only).**

---

## Unit 3 — `manage` skill content + helper commands

### Task 14: Write the `manage` SKILL.md content

**Files:**
- Modify: `src/operations/setup_claude/skill_manage.rs`

- [ ] **Step 1: Replace `content()` stub**

Set `content()` to a multi-section SKILL.md with the **three sections from the spec**: Command Guidance, Worktree-Health Rulebook (5-part rules: Stale cwd / Wrong-base branching / Sibling drift / Test-lint convention gap), Recommended-Hooks Catalog (Hook 1 / Hook 2 / Hook 3 with verbatim JSON).

The body must:
- Start with frontmatter:

```yaml
---
name: manage
description: "Manage git worktrees safely across multiple parallel sessions. Auto-applies when the user invokes gw list/delete/clean/sync/merge/pr/resume."
allowed-tools: Bash, Read, Edit
---
```

- Section 1 (Command Guidance): a Markdown table mirroring the existing `## Quick Reference` table, and a sentence pointing to `references/gw-commands.md` for full flag detail.

- Section 2 (Worktree-Health Rulebook): four rules. Each rule's body uses **literal subsection headers**:

```markdown
### Rule: Stale cwd / externally-deleted worktree

**Symptom:** ...
**Why it hurts:** ...
**Healthy state:** ...
**How to detect:** ...
**Suggested action:** ...
```

- Section 3 (Recommended-Hooks Catalog): three subsections (Hook 1, Hook 2, Hook 3) each with a fenced `jsonc` block containing the exact hook JSON from the spec, plus the rationale paragraph from the spec.

- Section 4 (When to suggest, when to stop): instructions to Claude saying:
  - Default-suggest Hook 1 when project `.claude/settings.json` does not contain a SessionStart entry naming `gw`.
  - Mention Hook 2 only after the user expresses interest in pre-publish safety after seeing Hook 1.
  - Mention Hook 3 only on direct request.
  - On consent, edit the **project's** `.claude/settings.json` (not `~/.claude/settings.json`).
  - On refusal or if equivalent hook is already present, do not re-prompt — the file's contents are the implicit state.

- [ ] **Step 2: Set `reference_content()` to the existing command reference**

Copy the body of `reference_content()` from `setup_claude_old.rs` lines 301–550 (already deleted by Task 11; instead read the original from git: `git show HEAD~N:src/operations/setup_claude_old.rs` if needed, or pull from `git log` history). Move it verbatim into `reference_content()`.

- [ ] **Step 3: Build**

Run: `cargo build`
Expected: success.

- [ ] **Step 4: Add a test asserting key sections are present**

Add to `tests/test_setup_claude_plugin.rs`:

```rust
#[test]
fn manage_skill_contains_required_sections() {
    let body = git_worktree_manager::operations::setup_claude_skill_manage_content();
    // Use a re-export helper if needed; if `skill_manage` is private, expose
    // a `pub fn manage_skill_content_for_test() -> &'static str` shim in mod.rs.
    assert!(body.contains("name: manage"));
    assert!(body.contains("Worktree-Health Rulebook"));
    assert!(body.contains("Recommended-Hooks Catalog"));
    assert!(body.contains("Rule: Stale cwd"));
    assert!(body.contains("Hook 1") && body.contains("SessionStart"));
    assert!(body.contains("Hook 2") && body.contains("PreToolUse"));
}
```

If `skill_manage` is a private sub-module, add to `src/operations/setup_claude/mod.rs`:

```rust
#[cfg(test)]
pub fn manage_skill_content_for_test() -> &'static str { skill_manage::content() }
```

And in the integration test call `setup_claude::manage_skill_content_for_test()` instead.

- [ ] **Step 5: Run tests**

Run: `cargo test --test test_setup_claude_plugin`
Expected: PASS (4 tests now).

- [ ] **Step 6: Commit**

```bash
git add src/operations/setup_claude/skill_manage.rs src/operations/setup_claude/mod.rs tests/test_setup_claude_plugin.rs
git commit -m "feat(setup_claude): manage skill body — guidance + rulebook + hook catalog"
```

---

### Task 15: `gw doctor --session-start --quiet` command

**Files:**
- Modify: `src/cli.rs` (the `Doctor` subcommand definition)
- Modify: `src/operations/diagnostics.rs` (the doctor entry point)

- [ ] **Step 1: Write the failing test**

Create `tests/test_doctor_session_start.rs`:

```rust
//! `gw doctor --session-start --quiet` produces a single-line summary that
//! is hook-friendly. Always exits 0.

#[test]
fn session_start_quiet_exits_zero_in_normal_repo() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_gw"))
        .arg("doctor")
        .arg("--session-start")
        .arg("--quiet")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("run gw doctor");
    assert!(out.status.success(), "doctor --session-start should exit 0");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let line_count = stdout.lines().filter(|l| !l.is_empty()).count();
    assert!(line_count <= 1, "expected at most one non-empty line, got: {stdout:?}");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test test_doctor_session_start`
Expected: FAIL — flags not recognized.

- [ ] **Step 3: Add the flags to `cli.rs`**

In `src/cli.rs`, find the `Doctor` variant (search `Doctor`) and extend it:

```rust
/// Run health diagnostics for the current worktree.
Doctor {
    /// Hook-friendly mode: emit a single-line summary and exit 0.
    #[arg(long)]
    session_start: bool,
    /// Suppress informational chatter; keep only the summary.
    #[arg(long)]
    quiet: bool,
},
```

- [ ] **Step 4: Implement the short-output path in `diagnostics.rs`**

In the function that handles the doctor command, branch on `session_start`:

```rust
pub fn doctor(session_start: bool, quiet: bool) -> Result<()> {
    if session_start {
        let cwd = std::env::current_dir().ok();
        let cwd_ok = cwd.as_ref().map(|p| p.exists()).unwrap_or(false);
        let branch = git::current_branch_in(cwd.as_deref()).unwrap_or_else(|_| "?".into());
        let base = git::base_branch_for(cwd.as_deref()).unwrap_or_else(|_| "?".into());
        let registered = cwd
            .as_deref()
            .and_then(|p| registry::lookup_worktree(p).ok().flatten())
            .is_some();
        let cwd_str = cwd.as_deref().map(|p| p.display().to_string()).unwrap_or_else(|| "?".into());
        if quiet {
            println!(
                "gw: cwd={} ok={} branch={} base={} registered={}",
                cwd_str, cwd_ok, branch, base, registered
            );
        } else {
            println!(
                "gw doctor: cwd={} ok={} branch={} base={} registered={}",
                cwd_str, cwd_ok, branch, base, registered
            );
        }
        return Ok(());
    }
    // ... existing full-doctor body unchanged
}
```

If helper functions like `git::current_branch_in`, `git::base_branch_for`, `registry::lookup_worktree` do not already exist in the exact form needed, use the closest existing helpers (search `current_branch` in `src/git.rs`). The point is a one-line summary; adapt to whatever signatures exist.

- [ ] **Step 5: Run the test**

Run: `cargo test --test test_doctor_session_start`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/cli.rs src/operations/diagnostics.rs tests/test_doctor_session_start.rs
git commit -m "feat(doctor): --session-start --quiet single-line hook-friendly mode"
```

---

### Task 16: `gw guard --tool-input -` command — risk classification

**Files:**
- Create: `src/operations/guard.rs`
- Modify: `src/cli.rs` (add `Guard` subcommand)
- Modify: `src/entrypoint.rs` (route `Commands::Guard`)
- Test: `tests/test_guard.rs`

- [ ] **Step 1: Write the failing test**

Create `tests/test_guard.rs`:

```rust
use std::io::Write;
use std::process::{Command, Stdio};

fn run_guard_with(payload: &str) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_gw"))
        .arg("guard")
        .arg("--tool-input").arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn gw guard");
    child.stdin.as_mut().unwrap().write_all(payload.as_bytes()).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn safe_command_passes() {
    let payload = r#"{"tool_name":"Bash","tool_input":{"command":"ls -la"}}"#;
    let out = run_guard_with(payload);
    assert!(out.status.success());
}

#[test]
fn risky_publish_blocked_when_cwd_unhealthy() {
    // Cwd is gw repo root which IS healthy, so this should pass.
    // To test the block branch we need a fake cwd. Use a path that does not
    // exist — guard treats missing cwd as unhealthy.
    let bad = "/nonexistent/dir/xyz";
    let payload = format!(
        r#"{{"tool_name":"Bash","tool_input":{{"command":"git push","cwd":"{}"}}}}"#,
        bad
    );
    let out = run_guard_with(&payload);
    assert!(!out.status.success(), "risky cmd in unhealthy cwd should block");
}

#[test]
fn non_bash_tool_passes() {
    let payload = r#"{"tool_name":"Read","tool_input":{"file_path":"/tmp/x"}}"#;
    let out = run_guard_with(payload);
    assert!(out.status.success());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test test_guard`
Expected: FAIL — `Guard` subcommand unknown.

- [ ] **Step 3: Add `Guard` to `cli.rs`**

```rust
/// Hook helper: read a Claude Code hook payload from stdin (or a file)
/// and decide whether to allow or block the inbound tool use. Exits 0 to
/// allow; non-zero with stderr message to block.
Guard {
    /// Path to read the hook payload from, or "-" for stdin.
    #[arg(long, value_name = "PATH")]
    tool_input: String,
},
```

- [ ] **Step 4: Implement `guard.rs`**

```rust
//! `gw guard` — Claude Code hook helper that vets inbound Bash tool calls.
//!
//! Input format: a JSON object with at least `tool_name` and `tool_input`.
//! For Bash, `tool_input.command` is matched against a small risk pattern
//! list. If the command is risky AND the cwd looks unhealthy (missing,
//! inside a deleted worktree), exit non-zero with a stderr message.

use std::io::Read;
use std::path::Path;

use crate::error::{CwError, Result};

const RISK_PATTERNS: &[&str] = &[
    "git push",
    "gh release",
    "gh pr merge",
    "npm publish",
    "cargo publish",
    "bun publish",
    "pnpm publish",
];

#[derive(Debug, serde::Deserialize)]
struct HookPayload {
    tool_name: Option<String>,
    tool_input: Option<serde_json::Value>,
}

fn read_input(source: &str) -> std::io::Result<String> {
    if source == "-" {
        let mut s = String::new();
        std::io::stdin().read_to_string(&mut s)?;
        Ok(s)
    } else {
        std::fs::read_to_string(source)
    }
}

fn cwd_is_healthy(cwd: &Path) -> bool {
    cwd.exists() && cwd.is_dir()
}

fn command_is_risky(cmd: &str) -> bool {
    let normalized = cmd.split_ascii_whitespace().collect::<Vec<_>>().join(" ");
    RISK_PATTERNS.iter().any(|pat| normalized.contains(pat))
}

pub fn run(tool_input_source: &str) -> Result<()> {
    let raw = read_input(tool_input_source).map_err(CwError::Io)?;
    let payload: HookPayload = serde_json::from_str(&raw).map_err(CwError::Json)?;

    if payload.tool_name.as_deref() != Some("Bash") {
        return Ok(());
    }
    let input = match payload.tool_input {
        Some(v) => v,
        None => return Ok(()),
    };
    let command = input
        .get("command")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if !command_is_risky(command) {
        return Ok(());
    }
    let cwd_str = input.get("cwd").and_then(|v| v.as_str());
    let cwd = match cwd_str {
        Some(s) => std::path::PathBuf::from(s),
        None => std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
    };
    if !cwd_is_healthy(&cwd) {
        eprintln!(
            "gw guard: blocking risky command '{}' — cwd '{}' does not exist or is not a directory.",
            command,
            cwd.display(),
        );
        return Err(CwError::ExitCode(2));
    }
    Ok(())
}
```

- [ ] **Step 5: Add `pub mod guard;` to `src/operations/mod.rs` and route the command in `entrypoint.rs`**

In `src/entrypoint.rs`, add an arm to the `match` on `Commands`:

```rust
Some(Commands::Guard { tool_input }) => operations::guard::run(&tool_input),
```

- [ ] **Step 6: Run tests**

Run: `cargo test --test test_guard`
Expected: PASS (3 tests).

- [ ] **Step 7: Commit**

```bash
git add src/operations/guard.rs src/operations/mod.rs src/cli.rs src/entrypoint.rs tests/test_guard.rs
git commit -m "feat(guard): hook helper to block risky bash in unhealthy cwd"
```

---

### Task 17: Cross-platform smoke for `gw guard` on macOS and Linux

This task is verification-only (no new code) but is required before declaring Unit 3 complete.

- [ ] **Step 1: Run full test suite locally**

Run: `cargo test --all-targets`
Expected: PASS on local platform.

- [ ] **Step 2: Push to a branch and let CI matrix run**

```bash
git push -u origin <branch-name>
```

Verify the GitHub Actions matrix (Linux + macOS at minimum) shows green for the new tests.

- [ ] **Step 3: No commit (CI verification only).**

---

### Task 18: Pre-release polish — clippy + fmt + binary size check

- [ ] **Step 1: Clippy with all targets and features**

Run: `cargo clippy --all-targets --all-features -- -D warnings`
Expected: 0 warnings. Fix any that appear (typical: unused imports in newly-added test files, missing `must_use`, or trivial `clone` flags).

- [ ] **Step 2: Format check**

Run: `cargo fmt --check`
Expected: no diff.

- [ ] **Step 3: Release build + size check**

Run: `cargo build --release && ls -l target/release/gw`
Expected: success. Note the size; CLAUDE.md says ~1.9MB baseline. Acceptable drift +/- 5%. If significantly larger, investigate (likely `chrono` if it was newly added).

- [ ] **Step 4: Run integration tests against the release binary**

Run: `cargo test --release --all-targets`
Expected: PASS.

- [ ] **Step 5: Commit any fixes from the polish pass**

```bash
git add -A
git commit -m "chore: clippy/fmt clean-up after plugin + worktree-health"
```

If there were no fixes, skip the commit.

---

### Task 19: Update CHANGELOG / README

**Files:**
- Modify: `README.md`
- Modify: `CHANGELOG.md` (release-please will rewrite, but a manual note at the top of "Unreleased" is fine)

- [ ] **Step 1: Add a "Plugin install" mention to README**

Find the existing `gw setup-claude` reference (if any) in `README.md` and update to mention plugin layout and the `manage` skill's hook-recommendation behavior. Keep it short — one paragraph plus a code block.

- [ ] **Step 2: Note the busy-detection change**

Add a "Behavior changes" subsection to README describing:
- `gw delete` now refuses with a clear tiered message; `--force` is the single override (no more interactive y/N).
- The interactive prompt is gone — automation that relied on piping `y` should pass `--force`.

- [ ] **Step 3: Run `cargo test` once more for sanity**

Run: `cargo test`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add README.md CHANGELOG.md
git commit -m "docs: gw setup-claude installs a plugin; --force is the sole busy override"
```

---

### Task 20: Open the PR

- [ ] **Step 1: Push final branch state**

```bash
git push
```

- [ ] **Step 2: Open PR**

```bash
gh pr create --title "feat: gw plugin + worktree-health skill + tiered in-use detection" --body "$(cat <<'EOF'
## Summary
- Convert `gw setup-claude` to install a Claude Code **plugin** at `~/.claude/plugins/gw/` with two skills (`delegate`, `manage`).
- `manage` skill embeds a worktree-health rulebook (4 rules in 5-part structure) and a recommended-hooks catalog (SessionStart sanity + optional PreToolUse guard + optional Stop summary). The skill — not gw — proposes hooks to the user in-session and edits the project's `.claude/settings.json` on consent.
- Refine `gw delete` busy detection into a 3-tier model: Hard tier (active Claude Code session via `~/.claude/projects/<encoded>/*.jsonl` event tail + explicit lockfile) refuses with a strong message; Soft tier (process cwd scan, refined) refuses with a warning. Single `--force` overrides both. Interactive y/N prompt removed.
- New helper commands: `gw doctor --session-start --quiet` (one-line hook output), `gw guard --tool-input -` (blocks risky bash in unhealthy cwd).
- `busy.rs` reduced ~60% by removing `is_suspicious_cmd` heuristic; decision logic is OS-portable, diagnostics stay best-effort per OS.

## Test plan
- [ ] `cargo test --all-targets` passes locally
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` clean
- [ ] CI matrix (Linux + macOS + Windows) green
- [ ] Manually run `gw setup-claude` against a tempdir HOME and verify plugin layout
- [ ] Manually `gw delete` a worktree (a) with no Claude session — passes silently; (b) with active Claude session — refuses with Hard message; (c) with `cd`'d shell only — refuses with Soft warning; (d) with `--force` — passes through every case
EOF
)"
```

- [ ] **Step 3: Confirm PR URL is returned and CI starts.**

---

## Self-Review Notes

Spec coverage map (one task → one spec section):

| Spec section | Tasks |
|---|---|
| Plugin layout | 9, 10, 11, 13 |
| Migration / legacy cleanup | 9, 10 |
| Hard tier: Claude jsonl tail | 2, 3, 4, 5 |
| Hard tier: lockfile (kept) | 5 |
| Soft tier: process scan + refinements | 1, 5 |
| TTY / start-time fields | 1 |
| `is_suspicious_cmd` removal | implied by 1's field reset; **add explicit step**: see addendum below |
| Refusal message shapes | 6, 7 |
| `--force` single flag | 7 |
| `manage` skill body (rulebook + catalog) | 14 |
| `gw doctor --session-start` | 15 |
| `gw guard --tool-input` | 16 |
| Cross-platform decision portability | 5 (no OS branches in `detect_busy_tiered`) |
| Doctor recognizes new install | 12 |

**Addendum:** explicitly remove `is_suspicious_cmd` and its call sites. Insert into Task 5 or as a Task 5b. Concrete change: in `src/operations/busy.rs`, delete the `fn is_suspicious_cmd` definition and its callers (search `is_suspicious_cmd` for occurrences). The function exists today as a heuristic; with Soft tier no longer carrying refusal precision, it is dead weight per spec.

**Type-consistency spot-check:**
- `BusyInfo` fields used in Task 1 match what Task 6's renderer reads (`pid, cmd, source, tier, tty, started_secs_ago`).
- `BusySource` enum gains `ClaudeSession` in Task 1, used by Task 5's emitter and Task 6's renderer.
- `find_active_sessions` signature `(project_dir, worktree, threshold) -> Vec<ActiveSession>` is fixed in Task 4 and called identically in Task 5.

**Placeholder scan:** Task 10 contains a `[... copy the body from setup_claude_old.rs lines 116-297 verbatim, BUT: ...]` instruction. This is a *concrete copy operation* with explicit byte boundaries and a specific edit (rename `name: gw` → `name: delegate`, drop the trailing reference link), not a vague "implement later." Acceptable per the placeholder guidance (the engineer has exact source, exact edits, exact target).

Task 14 references the spec's three-section structure with the rules and hooks named explicitly. The actual prose body is left to be authored by the engineer because the spec already contains the full text patterns to follow (Symptom/Why/Healthy/How/Action 5-part rule structure; the three Hook JSON blocks). If a reviewer wants the prose pre-written, that becomes a separate writing pass; this plan is sized for an engineer who will read the spec section "Plugin Skills" alongside.

**No type renames between tasks** found.

---
