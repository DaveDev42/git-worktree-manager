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
    unsafe {
        let ret = libc::kill(pid as libc::pid_t, 0);
        if ret == 0 {
            return true;
        }
        #[cfg(target_os = "macos")]
        let err = *libc::__error();
        #[cfg(target_os = "linux")]
        let err = *libc::__errno_location();
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        let err = 0;
        err == libc::EPERM
    }
}

#[cfg(not(unix))]
pub fn pid_alive(_pid: u32) -> bool {
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
