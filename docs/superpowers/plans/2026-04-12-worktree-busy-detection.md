# Worktree Busy Detection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `gw delete`/`gw clean`이 다른 세션(Claude Code, shell, 에디터 등)이 사용 중인 worktree를 실수로 삭제하지 않도록 하이브리드 busy 감지(lockfile + 프로세스 cwd 스캔)를 추가한다.

**Architecture:** 두 개의 새 모듈(`lockfile`, `busy`)을 `src/operations/` 아래에 만들고, 삭제 경로(`worktree::delete_worktree`, `clean::clean_worktrees`)와 상태 표시(`display::get_worktree_status`)에 통합한다. shell/AI 런처 진입 시 RAII로 lockfile을 획득해 명시적 세션을 표시하고, 외부에서 들어온 프로세스는 플랫폼별 cwd 스캔(macOS: `lsof`, Linux: `/proc`)으로 감지한다. 자기 자신과 조상 프로세스는 제외해 오탐을 막는다.

**Tech Stack:** Rust, `std::process::Command`, `libc` (PID liveness, getppid), `std::io::IsTerminal`, `serde_json` (lockfile 포맷)

Spec: `docs/superpowers/specs/2026-04-12-worktree-busy-detection-design.md`

---

## File Structure

**Create:**
- `src/operations/lockfile.rs` — `SessionLock` RAII guard, `acquire`/`read`, stale PID 정리
- `src/operations/busy.rs` — `BusyInfo`, `BusySource`, `detect_busy`, 자기 프로세스 트리 제외, 플랫폼별 cwd 스캔
- `tests/busy_detection.rs` — 통합 테스트 (임시 프로세스 spawn → 탐지)

**Modify:**
- `src/operations/mod.rs` — 새 모듈 등록
- `src/operations/display.rs` — `get_worktree_status`에 `"busy"` 우선순위 추가
- `src/operations/worktree.rs` — `delete_worktree`에서 busy 체크 + TTY 분기 + `--force` 처리
- `src/operations/clean.rs` — busy 자동 스킵, 요약 출력
- `src/operations/shell.rs` — shell 진입 시 `SessionLock::acquire`
- `src/operations/ai_tools.rs` — AI 런처 진입 시 `SessionLock::acquire`
- `src/cli.rs` — `delete`/`clean`에 이미 있는 `--force` 재사용 확인

---

## Task 1: Lockfile 모듈 스캐폴딩 + 단위 테스트

**Files:**
- Create: `src/operations/lockfile.rs`
- Modify: `src/operations/mod.rs`

- [ ] **Step 1: Register the new module**

Edit `src/operations/mod.rs` — add `pub mod lockfile;` after `pub mod launchers;`:

```rust
/// Operations module — business logic for all commands.
pub mod ai_tools;
pub mod backup;
pub mod clean;
pub mod config_ops;
pub mod diagnostics;
pub mod display;
pub mod git_ops;
pub mod global_ops;
pub mod helpers;
pub mod launchers;
pub mod lockfile;
pub mod path_cmd;
pub mod setup_claude;
pub mod shell;
pub mod stash;
pub mod worktree;
```

- [ ] **Step 2: Create the lockfile module with types + PID helpers**

Create `src/operations/lockfile.rs`:

```rust
//! Session lockfile — explicit "this worktree is in use" marker.
//!
//! Written when a user enters a worktree via `gw shell` or `gw start`.
//! Removed on Drop. Readers verify PID liveness and delete stale files.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::{CwError, Result};

const LOCK_FILENAME: &str = "gw-session.lock";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LockEntry {
    pub pid: u32,
    pub started_at: i64,
    pub cmd: String,
}

pub struct SessionLock {
    path: PathBuf,
}

impl Drop for SessionLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// Check whether a process with the given PID is currently alive.
#[cfg(unix)]
pub fn pid_alive(pid: u32) -> bool {
    // kill(pid, 0) returns 0 if the process exists and we can signal it,
    // or if it exists but we lack permission (errno == EPERM).
    unsafe {
        let ret = libc::kill(pid as libc::pid_t, 0);
        if ret == 0 {
            return true;
        }
        let err = *libc::__error();
        err == libc::EPERM
    }
}

#[cfg(not(unix))]
pub fn pid_alive(_pid: u32) -> bool {
    // Non-unix: assume alive (we don't produce lockfiles on these platforms).
    true
}

fn lock_path(worktree: &Path) -> PathBuf {
    worktree.join(".git").join(LOCK_FILENAME)
}

fn now_epoch_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Acquire a session lock for the given worktree. Dropping the returned
/// guard removes the file. If a stale lockfile (dead PID) exists, it is
/// overwritten. If a live lock exists, returns an error.
pub fn acquire(worktree: &Path, cmd: &str) -> Result<SessionLock> {
    let path = lock_path(worktree);

    if let Some(existing) = read(worktree) {
        if existing.pid != std::process::id() {
            return Err(CwError::Other(format!(
                "worktree already locked by PID {} ({})",
                existing.pid, existing.cmd
            )));
        }
    }

    let entry = LockEntry {
        pid: std::process::id(),
        started_at: now_epoch_seconds(),
        cmd: cmd.to_string(),
    };
    let json = serde_json::to_string(&entry)
        .map_err(|e| CwError::Other(format!("serialize lock: {}", e)))?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, json)?;
    Ok(SessionLock { path })
}

/// Read the current lockfile. Returns `None` if missing or stale.
/// Stale lockfiles are removed as a side effect.
pub fn read(worktree: &Path) -> Option<LockEntry> {
    let path = lock_path(worktree);
    let raw = fs::read_to_string(&path).ok()?;
    let entry: LockEntry = serde_json::from_str(&raw).ok()?;
    if pid_alive(entry.pid) {
        Some(entry)
    } else {
        let _ = fs::remove_file(&path);
        None
    }
}
```

- [ ] **Step 3: Add CwError::Other variant if not present**

Run `grep -n 'Other' src/error.rs` to confirm. If missing, add it.

```bash
grep -n 'Other\|pub enum CwError' /Users/dave/Projects/github.com/git-worktree-manager/src/error.rs
```

Expected: `Other(String)` variant exists. If not, edit `src/error.rs` to add:

```rust
    #[error("{0}")]
    Other(String),
```

- [ ] **Step 4: Write unit tests for lockfile**

Append to `src/operations/lockfile.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_worktree() -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join(".git")).unwrap();
        dir
    }

    #[test]
    fn acquire_writes_file_and_drop_removes_it() {
        let wt = make_worktree();
        let path = wt.path().join(".git").join(LOCK_FILENAME);
        {
            let _lock = acquire(wt.path(), "test").unwrap();
            assert!(path.exists());
        }
        assert!(!path.exists());
    }

    #[test]
    fn read_returns_entry_for_live_pid() {
        let wt = make_worktree();
        let _lock = acquire(wt.path(), "shell").unwrap();
        let entry = read(wt.path()).unwrap();
        assert_eq!(entry.pid, std::process::id());
        assert_eq!(entry.cmd, "shell");
    }

    #[test]
    fn read_removes_stale_lockfile() {
        let wt = make_worktree();
        // Write a lockfile with a PID that is almost certainly dead.
        let path = wt.path().join(".git").join(LOCK_FILENAME);
        let entry = LockEntry {
            pid: 999_999_999,
            started_at: 0,
            cmd: "ghost".to_string(),
        };
        fs::write(&path, serde_json::to_string(&entry).unwrap()).unwrap();
        assert!(read(wt.path()).is_none());
        assert!(!path.exists());
    }

    #[test]
    fn acquire_fails_when_live_lock_from_other_pid() {
        let wt = make_worktree();
        let path = wt.path().join(".git").join(LOCK_FILENAME);
        // Simulate a live lock from parent PID (always alive during test).
        let other_pid = unsafe { libc::getppid() } as u32;
        let entry = LockEntry {
            pid: other_pid,
            started_at: 0,
            cmd: "other".to_string(),
        };
        fs::write(&path, serde_json::to_string(&entry).unwrap()).unwrap();
        assert!(acquire(wt.path(), "shell").is_err());
    }
}
```

- [ ] **Step 5: Run lockfile tests**

Run: `cargo test -p git-worktree-manager --lib operations::lockfile`
Expected: `test result: ok. 4 passed`

- [ ] **Step 6: Commit**

```bash
git add src/operations/mod.rs src/operations/lockfile.rs src/error.rs
git commit -m "feat: add session lockfile with RAII guard and PID liveness check"
```

---

## Task 2: Busy detection module (self-exclusion + platform cwd scan)

**Files:**
- Create: `src/operations/busy.rs`
- Modify: `src/operations/mod.rs`

- [ ] **Step 1: Register module**

Edit `src/operations/mod.rs` — add `pub mod busy;` alphabetically:

```rust
pub mod ai_tools;
pub mod backup;
pub mod busy;
pub mod clean;
...
```

- [ ] **Step 2: Create busy.rs with types and self-exclusion logic**

Create `src/operations/busy.rs`:

```rust
//! Busy detection: determine whether a worktree is currently in use.
//!
//! Two signals are combined:
//!   1. Session lockfile (explicit — `gw shell`/`gw start` write one)
//!   2. Process cwd scan (implicit — catches external `cd` + tool usage)
//!
//! The current process and its ancestor chain are excluded so that Claude
//! Code or a parent shell invoking `gw delete` on its own worktree does
//! not self-detect as busy.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::lockfile;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BusySource {
    Lockfile,
    ProcessScan,
}

#[derive(Debug, Clone)]
pub struct BusyInfo {
    pub pid: u32,
    pub cmd: String,
    pub cwd: PathBuf,
    pub source: BusySource,
}

/// Returns the current process + all ancestor PIDs (via getppid chain).
#[cfg(unix)]
pub fn self_process_tree() -> HashSet<u32> {
    let mut tree = HashSet::new();
    tree.insert(std::process::id());

    // Current process's parent comes from libc::getppid() directly.
    let mut pid = unsafe { libc::getppid() } as u32;
    // Walk ancestors by reading each PID's parent. Limit loop to avoid
    // infinite cycles if the OS reports odd values.
    for _ in 0..64 {
        if pid == 0 || pid == 1 {
            tree.insert(pid);
            break;
        }
        tree.insert(pid);
        match parent_of(pid) {
            Some(ppid) if ppid != pid => pid = ppid,
            _ => break,
        }
    }
    tree
}

#[cfg(not(unix))]
pub fn self_process_tree() -> HashSet<u32> {
    let mut tree = HashSet::new();
    tree.insert(std::process::id());
    tree
}

#[cfg(target_os = "linux")]
fn parent_of(pid: u32) -> Option<u32> {
    let status = std::fs::read_to_string(format!("/proc/{}/status", pid)).ok()?;
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("PPid:") {
            return rest.trim().parse().ok();
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn parent_of(pid: u32) -> Option<u32> {
    let out = Command::new("ps")
        .args(["-o", "ppid=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn parent_of(_pid: u32) -> Option<u32> {
    None
}

/// Detect busy processes for a given worktree path.
///
/// Combines lockfile signal and process cwd scan. Filters out the current
/// process tree so that `gw delete` invoked from within the worktree does
/// not self-report as busy.
pub fn detect_busy(worktree: &Path) -> Vec<BusyInfo> {
    let exclude = self_process_tree();
    let mut out = Vec::new();

    if let Some(entry) = lockfile::read(worktree) {
        if !exclude.contains(&entry.pid) {
            out.push(BusyInfo {
                pid: entry.pid,
                cmd: entry.cmd,
                cwd: worktree.to_path_buf(),
                source: BusySource::Lockfile,
            });
        }
    }

    for info in scan_cwd(worktree) {
        if exclude.contains(&info.pid) {
            continue;
        }
        if out.iter().any(|b| b.pid == info.pid) {
            continue;
        }
        out.push(info);
    }

    out
}

#[cfg(target_os = "linux")]
fn scan_cwd(worktree: &Path) -> Vec<BusyInfo> {
    let mut out = Vec::new();
    let canon_target = match worktree.canonicalize() {
        Ok(p) => p,
        Err(_) => return out,
    };
    let proc_dir = match std::fs::read_dir("/proc") {
        Ok(d) => d,
        Err(_) => return out,
    };
    for entry in proc_dir.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let pid: u32 = match name.parse() {
            Ok(n) => n,
            Err(_) => continue,
        };
        let cwd_link = entry.path().join("cwd");
        let cwd = match std::fs::read_link(&cwd_link) {
            Ok(p) => p,
            Err(_) => continue,
        };
        let cwd_canon = cwd.canonicalize().unwrap_or(cwd.clone());
        if cwd_canon.starts_with(&canon_target) {
            let cmd = std::fs::read_to_string(entry.path().join("comm"))
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            out.push(BusyInfo {
                pid,
                cmd,
                cwd: cwd_canon,
                source: BusySource::ProcessScan,
            });
        }
    }
    out
}

#[cfg(target_os = "macos")]
fn scan_cwd(worktree: &Path) -> Vec<BusyInfo> {
    let mut out = Vec::new();
    let canon_target = match worktree.canonicalize() {
        Ok(p) => p,
        Err(_) => return out,
    };
    // `lsof -a -d cwd -F pcn +D <path>` prints records of the form:
    //   p<pid>\nc<cmd>\nn<path>\n
    let output = match Command::new("lsof")
        .args([
            "-a",
            "-d",
            "cwd",
            "-F",
            "pcn",
            "+D",
            &canon_target.to_string_lossy(),
        ])
        .output()
    {
        Ok(o) => o,
        Err(_) => return out,
    };
    // lsof returns non-zero when nothing matches — that's fine, just parse stdout.
    let stdout = String::from_utf8_lossy(&output.stdout);

    let mut cur_pid: Option<u32> = None;
    let mut cur_cmd = String::new();
    for line in stdout.lines() {
        if let Some(rest) = line.strip_prefix('p') {
            if let (Some(pid), false) = (cur_pid, cur_cmd.is_empty()) {
                // previous record without 'n' line — skip
                let _ = pid;
            }
            cur_pid = rest.parse().ok();
            cur_cmd.clear();
        } else if let Some(rest) = line.strip_prefix('c') {
            cur_cmd = rest.to_string();
        } else if let Some(rest) = line.strip_prefix('n') {
            if let Some(pid) = cur_pid {
                let cwd = PathBuf::from(rest);
                out.push(BusyInfo {
                    pid,
                    cmd: cur_cmd.clone(),
                    cwd,
                    source: BusySource::ProcessScan,
                });
            }
        }
    }
    out
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn scan_cwd(_worktree: &Path) -> Vec<BusyInfo> {
    Vec::new()
}
```

- [ ] **Step 3: Unit tests for self-exclusion**

Append to `src/operations/busy.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn self_tree_contains_current_pid() {
        let tree = self_process_tree();
        assert!(tree.contains(&std::process::id()));
    }

    #[cfg(unix)]
    #[test]
    fn self_tree_contains_parent_pid() {
        let tree = self_process_tree();
        let ppid = unsafe { libc::getppid() } as u32;
        assert!(tree.contains(&ppid), "expected tree to contain ppid {}", ppid);
    }
}
```

- [ ] **Step 4: Run busy unit tests**

Run: `cargo test -p git-worktree-manager --lib operations::busy`
Expected: `test result: ok. 2 passed`

- [ ] **Step 5: Commit**

```bash
git add src/operations/mod.rs src/operations/busy.rs
git commit -m "feat: add busy detection with lockfile + process cwd scan"
```

---

## Task 3: Integration test — external process held cwd detected as busy

**Files:**
- Create: `tests/busy_detection.rs`

- [ ] **Step 1: Write integration test**

Create `tests/busy_detection.rs`:

```rust
//! Integration test: spawn a sleep process with cwd inside a worktree
//! and verify `detect_busy` finds it.

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod unix_only {
    use std::process::{Command, Stdio};
    use std::thread::sleep;
    use std::time::Duration;

    use git_worktree_manager::operations::busy::{detect_busy, BusySource};
    use tempfile::TempDir;

    #[test]
    fn external_process_with_cwd_in_worktree_is_detected() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".git")).unwrap();

        let mut child = Command::new("sleep")
            .arg("30")
            .current_dir(dir.path())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn sleep");

        // Give the OS a moment to register the cwd.
        sleep(Duration::from_millis(200));

        let infos = detect_busy(dir.path());
        let found = infos
            .iter()
            .any(|i| i.pid == child.id() && i.source == BusySource::ProcessScan);

        let _ = child.kill();
        let _ = child.wait();

        assert!(
            found,
            "expected to detect spawned child pid={} in {:?}",
            child.id(),
            infos
        );
    }

    #[test]
    fn no_busy_when_worktree_empty() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".git")).unwrap();
        let infos = detect_busy(dir.path());
        // Test runner itself is in self_tree and excluded; no other process
        // should have its cwd inside this fresh tempdir.
        assert!(infos.is_empty(), "unexpected busy: {:?}", infos);
    }
}
```

- [ ] **Step 2: Run integration tests**

Run: `cargo test --test busy_detection`
Expected: `test result: ok. 2 passed` on macOS/Linux.

- [ ] **Step 3: Commit**

```bash
git add tests/busy_detection.rs
git commit -m "test: integration test for external process busy detection"
```

---

## Task 4: Extend `get_worktree_status` with `busy` state

**Files:**
- Modify: `src/operations/display.rs` (the `get_worktree_status` function around line 20-75)

- [ ] **Step 1: Write a failing test for busy status**

Append to the existing `#[cfg(test)] mod tests` block near the bottom of `src/operations/display.rs` (search for `fn test_get_worktree_status_stale`):

```rust
    #[test]
    fn test_get_worktree_status_busy_from_lockfile() {
        use crate::operations::lockfile::{self, LockEntry};
        use std::fs;

        let tmp = tempfile::TempDir::new().unwrap();
        let repo = tmp.path();
        let wt = repo.join("wt1");
        fs::create_dir_all(wt.join(".git")).unwrap();

        // Write a lockfile owned by our parent PID (alive, not self).
        let ppid = unsafe { libc::getppid() } as u32;
        // If ppid happens to equal our pid (rare test runner setup), skip.
        if ppid == std::process::id() {
            return;
        }
        let entry = LockEntry {
            pid: ppid,
            started_at: 0,
            cmd: "claude".to_string(),
        };
        let _ = lockfile::read(&wt); // ensure import is live
        fs::write(
            wt.join(".git").join("gw-session.lock"),
            serde_json::to_string(&entry).unwrap(),
        )
        .unwrap();

        let status = get_worktree_status(&wt, repo, Some("wt1"));
        assert_eq!(status, "busy");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p git-worktree-manager --lib operations::display::tests::test_get_worktree_status_busy_from_lockfile`
Expected: FAIL — status returns something other than `"busy"` (likely `"clean"` or `"active"`).

- [ ] **Step 3: Add busy branch to `get_worktree_status`**

Edit `src/operations/display.rs`. Replace the body of `get_worktree_status` (line 28–75) with:

```rust
pub fn get_worktree_status(path: &Path, repo: &Path, branch: Option<&str>) -> String {
    if !path.exists() {
        return "stale".to_string();
    }

    // Busy beats "active": another session (claude, shell, editor) holds this
    // worktree. The current process and its ancestors are excluded inside
    // detect_busy so the caller's own shell does not self-report.
    if !crate::operations::busy::detect_busy(path).is_empty() {
        return "busy".to_string();
    }

    // Check if cwd is inside this worktree
    if let Ok(cwd) = std::env::current_dir() {
        let cwd_str = cwd.to_string_lossy().to_string();
        let path_str = path.to_string_lossy().to_string();
        if cwd_str.starts_with(&path_str) {
            return "active".to_string();
        }
    }

    // Check merge/PR status if branch name is available
    if let Some(branch_name) = branch {
        let base_branch = {
            let key = format_config_key(CONFIG_KEY_BASE_BRANCH, branch_name);
            git::get_config(&key, Some(repo))
                .unwrap_or_else(|| git::detect_default_branch(Some(repo)))
        };

        if let Some(pr_state) = git::get_pr_state(branch_name, Some(repo)) {
            match pr_state.as_str() {
                "MERGED" => return "merged".to_string(),
                "OPEN" => return "pr-open".to_string(),
                _ => {}
            }
        }

        if git::is_branch_merged(branch_name, &base_branch, Some(repo)) {
            return "merged".to_string();
        }
    }

    if let Ok(result) = git::git_command(&["status", "--porcelain"], Some(path), false, true) {
        if result.returncode == 0 && !result.stdout.trim().is_empty() {
            return "modified".to_string();
        }
    }

    "clean".to_string()
}
```

Also update the doc comment above it (line 20):

```rust
/// Status priority: stale > busy > active > merged > pr-open > modified > clean
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p git-worktree-manager --lib operations::display::tests::test_get_worktree_status_busy_from_lockfile`
Expected: PASS

- [ ] **Step 5: Update other display sites that enumerate status values**

Search: `grep -n '"active".*"pr-open"\|status_name' src/operations/display.rs`

Around line 164, update the status enumeration to include busy:

```rust
        for &status_name in &["clean", "modified", "busy", "active", "pr-open", "merged", "stale"] {
```

- [ ] **Step 6: Run the full display test suite**

Run: `cargo test -p git-worktree-manager --lib operations::display`
Expected: all display tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/operations/display.rs
git commit -m "feat: add busy status to worktree state with highest non-stale priority"
```

---

## Task 5: Wire `SessionLock` into `gw shell`

**Files:**
- Modify: `src/operations/shell.rs`

- [ ] **Step 1: Inspect current shell entrypoint**

Run: `grep -n 'pub fn\|exec\|spawn' /Users/dave/Projects/github.com/git-worktree-manager/src/operations/shell.rs | head -20`

Identify the function that launches the interactive shell (typically `pub fn enter_shell` or similar). Read it.

- [ ] **Step 2: Acquire lock before spawning the shell**

In the function that spawns the shell, immediately before the `Command::new(...).spawn()` or `.status()` call, add:

```rust
    let _session_lock = crate::operations::lockfile::acquire(&worktree_path, "shell")
        .map_err(|e| {
            eprintln!(
                "{} could not acquire session lock: {}",
                console::style("warning:").yellow(),
                e
            );
        })
        .ok();
```

The `_session_lock` binding lives until the function returns (after the shell process exits), so Drop removes the file automatically.

Replace `worktree_path` with the actual variable name in that function (e.g., `path`, `wt_path`).

- [ ] **Step 3: Build**

Run: `cargo build`
Expected: compiles with no warnings.

- [ ] **Step 4: Manual smoke test**

```bash
cargo build --release
# In one terminal:
./target/release/gw shell <some-worktree>
# In another terminal:
cat /path/to/worktree/.git/gw-session.lock
# Expected: JSON with current shell PID and cmd="shell"
# Exit the shell; the lockfile should be gone.
```

- [ ] **Step 5: Commit**

```bash
git add src/operations/shell.rs
git commit -m "feat: acquire session lockfile when entering gw shell"
```

---

## Task 6: Wire `SessionLock` into AI tool launchers

**Files:**
- Modify: `src/operations/ai_tools.rs`

- [ ] **Step 1: Inspect AI launcher entry**

Run: `grep -n 'pub fn\|launch\|spawn\|Command::new' /Users/dave/Projects/github.com/git-worktree-manager/src/operations/ai_tools.rs | head -30`

Identify the top-level dispatcher function (e.g., `launch_ai_tool`) that runs before delegating to `launchers/*`.

- [ ] **Step 2: Acquire lock in AI launcher dispatcher**

At the top of the function that receives the worktree path and tool name, add:

```rust
    let _session_lock = crate::operations::lockfile::acquire(&worktree_path, tool_name)
        .map_err(|e| {
            eprintln!(
                "{} could not acquire session lock: {}",
                console::style("warning:").yellow(),
                e
            );
        })
        .ok();
```

Use the actual parameter names (commonly `path`/`wt`/`worktree` for the path and `tool`/`ai_tool`/`cmd` for the tool name — pass a `&str`, e.g., `"claude"` or `"cursor"`).

- [ ] **Step 3: Build**

Run: `cargo build`
Expected: compiles with no warnings.

- [ ] **Step 4: Manual smoke test**

```bash
./target/release/gw start <some-worktree> --claude  # or whichever launcher flag your setup uses
# Before accepting the Claude prompt, in another terminal:
cat <worktree>/.git/gw-session.lock
# Expected: JSON with the launcher's PID and cmd="claude" (or similar)
```

- [ ] **Step 5: Commit**

```bash
git add src/operations/ai_tools.rs
git commit -m "feat: acquire session lockfile when launching AI tools"
```

---

## Task 7: `gw delete` — busy check + TTY branching + `--force`

**Files:**
- Modify: `src/operations/worktree.rs`

- [ ] **Step 1: Inspect `delete_worktree` signature**

Run: `grep -n 'pub fn delete_worktree' /Users/dave/Projects/github.com/git-worktree-manager/src/operations/worktree.rs`

Read the function. Note the existing `force: bool` parameter (the clean.rs call passes `false, false, true, None`).

- [ ] **Step 2: Add busy gate near the top of `delete_worktree`**

Inside `delete_worktree`, right after the worktree path is resolved and before any destructive git/fs calls, insert:

```rust
    use std::io::IsTerminal;

    let busy = crate::operations::busy::detect_busy(&worktree_path);
    if !busy.is_empty() && !force {
        eprintln!(
            "{} worktree '{}' is in use by:",
            console::style("error:").red().bold(),
            branch_or_name
        );
        for b in &busy {
            eprintln!(
                "    PID {:>6}  {}  (source: {:?})",
                b.pid, b.cmd, b.source
            );
        }

        if std::io::stdin().is_terminal() && std::io::stderr().is_terminal() {
            eprint!("Delete anyway? (y/N): ");
            use std::io::Write;
            let _ = std::io::stderr().flush();
            let mut buf = String::new();
            std::io::stdin().read_line(&mut buf)?;
            let ans = buf.trim().to_lowercase();
            if ans != "y" && ans != "yes" {
                eprintln!("Aborted.");
                return Ok(());
            }
        } else {
            return Err(crate::error::CwError::Other(format!(
                "worktree '{}' is in use by {} process(es); re-run with --force to override",
                branch_or_name,
                busy.len()
            )));
        }
    }
```

Replace `worktree_path` and `branch_or_name` with the actual variables in scope (likely `path` and the branch argument). If the branch name has not been resolved yet, move this block below that resolution.

- [ ] **Step 3: Ensure `force` is a real flag in CLI**

Run: `grep -n 'fn delete\|Delete\|force' /Users/dave/Projects/github.com/git-worktree-manager/src/cli.rs | head -20`

Confirm `--force` already exists on the `Delete` subcommand. If not, add a `#[arg(long)] force: bool` field.

- [ ] **Step 4: Build and test**

Run: `cargo build && cargo test -p git-worktree-manager --lib operations::worktree`
Expected: compiles, existing tests still pass.

- [ ] **Step 5: Write an integration-style test for non-TTY busy rejection**

Append to `tests/busy_detection.rs`:

```rust
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn gw_delete_rejects_busy_worktree_when_not_tty() {
    use assert_cmd::Command;
    use std::process::{Command as StdCommand, Stdio};
    use std::thread::sleep;
    use std::time::Duration;

    // This test assumes a helper script can spin up a fake worktree.
    // Because gw delete needs a real repo, this is a best-effort smoke:
    // we just assert that the binary returns non-zero when given a busy
    // marker via lockfile.
    let repo = tempfile::TempDir::new().unwrap();
    // Initialize git repo + add worktree. If this fails, skip.
    let init = StdCommand::new("git")
        .arg("init")
        .current_dir(repo.path())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    if init.map(|s| !s.success()).unwrap_or(true) {
        return; // skip; environment lacks git
    }

    // Make an initial commit so worktree add works.
    std::fs::write(repo.path().join("README"), "hi").unwrap();
    let _ = StdCommand::new("git")
        .args(["-c", "user.email=t@t", "-c", "user.name=t", "add", "."])
        .current_dir(repo.path())
        .status();
    let _ = StdCommand::new("git")
        .args(["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-m", "i"])
        .current_dir(repo.path())
        .status();

    // Create a worktree at ../wt-busy
    let wt_path = repo.path().parent().unwrap().join("wt-busy-test");
    let _ = std::fs::remove_dir_all(&wt_path);
    let add = StdCommand::new("git")
        .args([
            "worktree",
            "add",
            "-b",
            "busy-branch",
            wt_path.to_str().unwrap(),
        ])
        .current_dir(repo.path())
        .status();
    if add.map(|s| !s.success()).unwrap_or(true) {
        return;
    }

    // Hold cwd from an external sleep process.
    let mut child = StdCommand::new("sleep")
        .arg("30")
        .current_dir(&wt_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    sleep(Duration::from_millis(200));

    // Invoke gw delete with stdin piped (so it's not a TTY).
    let output = Command::cargo_bin("gw")
        .unwrap()
        .args(["delete", "busy-branch"])
        .current_dir(repo.path())
        .write_stdin("")
        .output()
        .unwrap();

    let _ = child.kill();
    let _ = child.wait();

    // Cleanup best-effort.
    let _ = StdCommand::new("git")
        .args(["worktree", "remove", "--force", wt_path.to_str().unwrap()])
        .current_dir(repo.path())
        .status();

    assert!(
        !output.status.success(),
        "expected gw delete to fail for busy worktree; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("in use") || stderr.contains("busy") || stderr.contains("--force"),
        "stderr should mention busy/force: {}",
        stderr
    );
}
```

- [ ] **Step 6: Run the new test**

Run: `cargo test --test busy_detection gw_delete_rejects_busy_worktree_when_not_tty`
Expected: PASS (or skipped if environment lacks `git`).

- [ ] **Step 7: Commit**

```bash
git add src/operations/worktree.rs tests/busy_detection.rs
git commit -m "feat(delete): block busy worktree deletion with TTY-aware prompt"
```

---

## Task 8: `gw clean` — auto-skip busy, summary

**Files:**
- Modify: `src/operations/clean.rs`

- [ ] **Step 1: Add `force` parameter to clean_worktrees**

Edit `src/operations/clean.rs` — update the signature at line 13:

```rust
pub fn clean_worktrees(
    merged: bool,
    older_than: Option<u64>,
    interactive: bool,
    dry_run: bool,
    force: bool,
) -> Result<()> {
```

Update the CLI call site. Run:
`grep -n 'clean_worktrees' /Users/dave/Projects/github.com/git-worktree-manager/src/main.rs /Users/dave/Projects/github.com/git-worktree-manager/src/cli.rs`

Find the dispatch and append `force` (plumb from a new `#[arg(long)] force: bool` on the Clean subcommand). If `--force` already exists on Clean, just pass it through.

- [ ] **Step 2: Filter busy entries before deletion**

After the `to_delete` vector is fully populated (currently just before the `if to_delete.is_empty()` check around line 115), insert:

```rust
    // Separate busy worktrees so we can report skipped count accurately.
    let mut busy_skipped: Vec<(String, Vec<crate::operations::busy::BusyInfo>)> = Vec::new();
    if !force {
        let mut kept: Vec<(String, String, String)> = Vec::with_capacity(to_delete.len());
        for (branch, path, reason) in to_delete.into_iter() {
            let busy = crate::operations::busy::detect_busy(std::path::Path::new(&path));
            if busy.is_empty() {
                kept.push((branch, path, reason));
            } else {
                busy_skipped.push((branch, busy));
            }
        }
        to_delete = kept;
    }
```

- [ ] **Step 3: Print busy-skipped summary**

Directly after the filter block, add:

```rust
    if !busy_skipped.is_empty() {
        println!(
            "{}",
            style(format!(
                "Skipping {} busy worktree(s) (use --force to override):",
                busy_skipped.len()
            ))
            .yellow()
        );
        for (branch, infos) in &busy_skipped {
            let first = infos.first();
            let detail = first
                .map(|b| format!("PID {} {}", b.pid, b.cmd))
                .unwrap_or_default();
            println!("  - {:<30} (busy: {})", branch, detail);
        }
        println!();
    }
```

- [ ] **Step 4: Build + run existing clean tests**

Run: `cargo build && cargo test -p git-worktree-manager clean`
Expected: compiles, all existing clean tests still pass (they just need the new `force: bool` arg at call sites).

- [ ] **Step 5: Commit**

```bash
git add src/operations/clean.rs src/main.rs src/cli.rs
git commit -m "feat(clean): skip busy worktrees and report summary, --force to override"
```

---

## Task 9: End-to-end manual verification

**Files:** none (manual)

- [ ] **Step 1: Build release binary**

Run: `cargo build --release`
Expected: `target/release/gw` produced, no warnings.

- [ ] **Step 2: Full test suite**

Run: `cargo test`
Expected: all tests pass (ignored count unchanged from baseline).

- [ ] **Step 3: Clippy clean**

Run: `cargo clippy --all-targets -- -D warnings`
Expected: no warnings.

- [ ] **Step 4: Format**

Run: `cargo fmt --check`
Expected: no diff.

- [ ] **Step 5: Manual scenario 1 — external claude holding worktree**

```bash
# Terminal A
cd <some-test-repo>
./target/release/gw create test-busy
cd <worktree-of-test-busy>
sleep 300 &     # stand-in for "claude running"

# Terminal B (back in main repo)
./target/release/gw clean -i
# Type 'test-busy' when prompted
# Expected: "Skipping 1 busy worktree(s)" summary, worktree NOT deleted.

./target/release/gw delete test-busy
# Expected: if TTY, prompt; if piped (echo "" | gw delete test-busy), error + non-zero exit.

./target/release/gw delete test-busy --force
# Expected: deletes despite busy.
```

- [ ] **Step 6: Manual scenario 2 — lockfile signal**

```bash
# Terminal A
./target/release/gw shell test-busy2
# Leave the shell running.

# Terminal B
./target/release/gw list
# Expected: test-busy2 shows 'busy' status.

./target/release/gw delete test-busy2
# Expected: busy rejection or prompt.

# Terminal A: exit the shell.
# Lockfile should be removed automatically.
```

- [ ] **Step 7: Commit any final tweaks and tag**

If any fixes were made during manual verification:

```bash
git add -A
git commit -m "fix: adjust busy detection per manual verification"
```

No tag — release-please handles versioning.

---

## Self-Review Pass (author note)

1. **Spec coverage:**
   - Lockfile (§1 signal A) → Task 1
   - Process scan + self-exclusion (§1 signal B) → Tasks 2, 3
   - `gw delete` TTY/non-TTY/`--force` (§2) → Task 7
   - `gw clean` skip + summary (§2) → Task 8
   - Status priority (§3) → Task 4
   - shell/ai_tools lockfile acquire (§4) → Tasks 5, 6
   - Tests (§6) → embedded in each task
2. **Placeholder scan:** no TBD/TODO/vague phrases.
3. **Type consistency:** `BusyInfo { pid, cmd, cwd, source }`, `BusySource::{Lockfile, ProcessScan}`, `LockEntry { pid, started_at, cmd }`, `SessionLock`, `detect_busy`, `lockfile::acquire/read` all used consistently across tasks.
