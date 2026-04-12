//! Session lockfile — explicit "this worktree is in use" marker.
//!
//! Written when a user enters a worktree via `gw shell` or `gw start`.
//! Removed on Drop. Readers verify PID liveness and delete stale files.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

const LOCK_FILENAME: &str = "gw-session.lock";

/// Non-unix stale-lock TTL: a lockfile whose mtime is older than this is
/// treated as belonging to a crashed process and removed on next read.
#[cfg(not(unix))]
const STALE_TTL: std::time::Duration = std::time::Duration::from_secs(7 * 24 * 60 * 60);

/// Current on-disk lockfile schema version. A mismatching version makes
/// the lockfile "foreign" for the reader — `read_and_clean_stale` returns
/// it as-is without removing the file. Writers still check PID ownership,
/// so a foreign-version entry held by another PID is treated as a live
/// foreign lock by `acquire` and refused.
pub const LOCK_VERSION: u32 = 1;

fn default_version() -> u32 {
    0
}

/// Serialized lockfile contents describing the session that owns a worktree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockEntry {
    #[serde(default = "default_version")]
    pub version: u32,
    pub pid: u32,
    pub started_at: i64,
    pub cmd: String,
}

/// RAII guard that removes the lockfile when dropped, provided the lockfile
/// still belongs to this process. A `Drop` that blindly removed the file
/// would delete another session's lock if two processes raced the acquire.
///
/// Note: the RAII guard only guarantees correct cleanup on drop. It does
/// **not** guarantee ongoing exclusive access — if another process bypasses
/// `acquire` and clobbers the file mid-session, this guard will not notice,
/// though Drop will correctly refuse to remove a file whose PID no longer
/// matches.
pub struct SessionLock {
    path: PathBuf,
    owner_pid: u32,
}

impl Drop for SessionLock {
    fn drop(&mut self) {
        // There is a microsecond-scale race here: between reading and
        // removing, another process could in theory write its own lockfile.
        // In practice, a foreigner would have had to pass acquire's
        // ownership check — which it could not while our (still-present)
        // lockfile named this PID. So this is best-effort and safe.
        // If the file is unreadable or malformed we cannot prove ownership —
        // leave it alone and let a subsequent `read_and_clean_stale` or
        // `acquire` handle it.
        if let Ok(raw) = fs::read_to_string(&self.path) {
            if let Ok(entry) = serde_json::from_str::<LockEntry>(&raw) {
                if entry.pid != self.owner_pid {
                    return;
                }
                let _ = fs::remove_file(&self.path);
            }
        }
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

/// Resolve the directory that should hold the lockfile.
///
/// In a git worktree created by `git worktree add`, `<worktree>/.git` is a
/// *file* containing `gitdir: <absolute-path>` pointing to the per-worktree
/// directory under `<main>/.git/worktrees/<name>`. Writing a lockfile inside
/// a regular file would fail silently, so we dereference the `gitdir:`
/// indicator. In the main worktree, `.git` is a directory and is used as-is.
fn lock_dir(worktree: &Path) -> PathBuf {
    let dot_git = worktree.join(".git");
    if let Ok(meta) = fs::metadata(&dot_git) {
        if meta.is_file() {
            if let Ok(raw) = fs::read_to_string(&dot_git) {
                for line in raw.lines() {
                    if let Some(rest) = line.strip_prefix("gitdir:") {
                        let trimmed = rest.trim();
                        if !trimmed.is_empty() {
                            return PathBuf::from(trimmed);
                        }
                    }
                }
            }
        }
    }
    dot_git
}

fn lock_path(worktree: &Path) -> PathBuf {
    lock_dir(worktree).join(LOCK_FILENAME)
}

fn now_epoch_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Outcome of an `acquire` call. Callers may want to distinguish "an active
/// session already owns this worktree" (should block entry) from "we could
/// not write the lockfile at all" (degrade to a warning and proceed).
#[derive(Debug, thiserror::Error)]
pub enum AcquireError {
    /// Another live session holds the lock.
    #[error("worktree already in use by PID {} ({})", .0.pid, .0.cmd)]
    ForeignLock(LockEntry),
    /// Lockfile could not be written/read.
    #[error("lockfile I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// Serializing/deserializing the lockfile failed.
    #[error("lockfile serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

/// Sweep the lock directory for stale tmp files left by dead processes.
/// Filenames have the form `gw-session.lock.tmp.<pid>`. Removal errors are
/// ignored — this is best-effort housekeeping.
fn cleanup_stale_tmp_files(dir: &Path) {
    let entries = match fs::read_dir(dir) {
        Ok(d) => d,
        Err(_) => return,
    };
    let prefix = format!("{}.tmp.", LOCK_FILENAME);
    let me = std::process::id();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_s = name.to_string_lossy();
        let Some(pid_str) = name_s.strip_prefix(&prefix) else {
            continue;
        };
        let Ok(pid) = pid_str.parse::<u32>() else {
            continue;
        };
        if pid == me {
            continue;
        }
        if !pid_alive(pid) {
            let _ = fs::remove_file(entry.path());
        }
    }
}

/// Acquire an exclusive session lock for the given worktree. Cleans up stale
/// locks; returns `AcquireError::ForeignLock` if a live foreign PID holds the
/// lock, or `AcquireError::Io`/`AcquireError::Serde` for I/O / serialization
/// failures.
pub fn acquire(worktree: &Path, cmd: &str) -> std::result::Result<SessionLock, AcquireError> {
    let path = lock_path(worktree);

    if let Some(existing) = read_and_clean_stale(worktree) {
        if existing.pid != std::process::id() {
            return Err(AcquireError::ForeignLock(existing));
        }
    }

    let entry = LockEntry {
        version: LOCK_VERSION,
        pid: std::process::id(),
        started_at: now_epoch_seconds(),
        cmd: cmd.to_string(),
    };
    let json = serde_json::to_string(&entry)?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
        // Remove any stale tmp files from dead PIDs before writing our own.
        cleanup_stale_tmp_files(parent);
    }

    // Atomic write: write to tmp, then rename. The tmp name includes our
    // PID so racing processes do not clobber each other's tmp files.
    let tmp = path.with_file_name(format!("{}.tmp.{}", LOCK_FILENAME, std::process::id()));
    {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp)?;
        f.write_all(json.as_bytes())?;
        f.sync_all().ok();
    }
    fs::rename(&tmp, &path)?;

    // Post-rename ownership verification: if two processes both passed the
    // pre-check and raced the rename, only one file survives on disk. Re-read
    // and confirm it still contains our PID; otherwise the other process won
    // the race and we must not return a SessionLock (whose Drop would then
    // remove the foreigner's file).
    if let Ok(raw) = fs::read_to_string(&path) {
        if let Ok(final_entry) = serde_json::from_str::<LockEntry>(&raw) {
            if final_entry.pid != std::process::id() {
                return Err(AcquireError::ForeignLock(final_entry));
            }
        }
    }

    Ok(SessionLock {
        path,
        owner_pid: std::process::id(),
    })
}

/// Read the current lock entry, cleaning up if the owner is gone.
///
/// Behavior by schema version:
/// - Foreign-version entries (e.g. written by a future `gw`) are returned
///   unmodified and never cleaned — we do not own them.
/// - Same-version entries are cleaned when the owner is known-dead:
///   on unix, `pid_alive(pid)` is the authority; on non-unix (where we
///   cannot cheaply verify PID liveness), the lockfile's mtime is the
///   fallback and a file older than 7 days is treated as stale.
pub fn read_and_clean_stale(worktree: &Path) -> Option<LockEntry> {
    let path = lock_path(worktree);
    let raw = fs::read_to_string(&path).ok()?;
    let entry: LockEntry = serde_json::from_str(&raw).ok()?;

    // Version gate: unknown versions are treated as foreign locks — do not
    // touch them beyond returning whatever we can parse. A future gw version
    // bumping LOCK_VERSION must still interoperate safely here.
    if entry.version != LOCK_VERSION {
        return Some(entry);
    }

    // Prefer OS-level liveness when we have it.
    #[cfg(unix)]
    let alive = pid_alive(entry.pid);
    // On non-unix we cannot cheaply verify PID liveness, so fall back to
    // mtime: if the lockfile has not been touched in STALE_TTL, assume the
    // owner crashed and clean it up. Metadata read failures bias toward
    // keeping the lockfile (report as alive) to avoid accidentally nuking a
    // real session over a transient filesystem glitch.
    #[cfg(not(unix))]
    let alive = match fs::metadata(&path).and_then(|m| m.modified()) {
        Ok(mtime) => std::time::SystemTime::now()
            .duration_since(mtime)
            .map(|age| age < STALE_TTL)
            .unwrap_or(true),
        Err(_) => true,
    };

    if alive {
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
        let entry = read_and_clean_stale(wt.path()).unwrap();
        assert_eq!(entry.pid, std::process::id());
        assert_eq!(entry.cmd, "shell");
    }

    // unix-only: relies on pid_alive returning false for a fake PID. The
    // non-unix implementation falls back to mtime, so a freshly written
    // lockfile is never considered stale in that path.
    #[cfg(unix)]
    #[test]
    fn read_removes_stale_lockfile() {
        let wt = make_worktree();
        let path = wt.path().join(".git").join(LOCK_FILENAME);
        let entry = LockEntry {
            version: LOCK_VERSION,
            pid: 999_999_999,
            started_at: 0,
            cmd: "ghost".to_string(),
        };
        fs::write(&path, serde_json::to_string(&entry).unwrap()).unwrap();
        assert!(read_and_clean_stale(wt.path()).is_none());
        assert!(!path.exists());
    }

    #[test]
    fn acquire_does_not_leave_tmp_file_behind() {
        let wt = make_worktree();
        let _lock = acquire(wt.path(), "shell").unwrap();
        let git_dir = wt.path().join(".git");
        let entries: Vec<_> = fs::read_dir(&git_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        let tmp_files: Vec<_> = entries
            .iter()
            .filter(|n| n.starts_with("gw-session.lock.tmp."))
            .collect();
        assert!(tmp_files.is_empty(), "tmp files leaked: {:?}", tmp_files);
        assert!(entries.iter().any(|n| n == "gw-session.lock"));
    }

    #[test]
    fn lock_dir_follows_gitdir_indicator_when_dot_git_is_file() {
        // Simulate a git worktree: <worktree>/.git is a file containing
        // `gitdir: <path>` pointing to the real per-worktree directory.
        let root = TempDir::new().unwrap();
        let real_gitdir = root.path().join("main.git/worktrees/feature");
        fs::create_dir_all(&real_gitdir).unwrap();
        let wt = root.path().join("feature");
        fs::create_dir_all(&wt).unwrap();
        fs::write(
            wt.join(".git"),
            format!("gitdir: {}\n", real_gitdir.display()),
        )
        .unwrap();

        let dir = lock_dir(&wt);
        assert_eq!(dir, real_gitdir);

        let _lock = acquire(&wt, "shell").unwrap();
        assert!(real_gitdir.join(LOCK_FILENAME).exists());
        let entry = read_and_clean_stale(&wt).unwrap();
        assert_eq!(entry.pid, std::process::id());
    }

    #[cfg(unix)]
    #[test]
    fn drop_does_not_remove_lockfile_owned_by_another_process() {
        let wt = make_worktree();
        let lock = acquire(wt.path(), "shell").unwrap();
        // Overwrite the file as if another process had taken over.
        let entry = LockEntry {
            version: LOCK_VERSION,
            pid: unsafe { libc::getppid() } as u32,
            started_at: 0,
            cmd: "other".to_string(),
        };
        let path = wt.path().join(".git").join(LOCK_FILENAME);
        fs::write(&path, serde_json::to_string(&entry).unwrap()).unwrap();

        drop(lock);
        assert!(
            path.exists(),
            "foreign-owned lockfile was incorrectly removed"
        );
        // Cleanup for the TempDir drop
        let _ = fs::remove_file(&path);
    }

    #[cfg(unix)]
    #[test]
    fn acquire_fails_when_live_lock_from_other_pid() {
        let wt = make_worktree();
        let path = wt.path().join(".git").join(LOCK_FILENAME);
        let other_pid = unsafe { libc::getppid() } as u32;
        let entry = LockEntry {
            version: LOCK_VERSION,
            pid: other_pid,
            started_at: 0,
            cmd: "other".to_string(),
        };
        fs::write(&path, serde_json::to_string(&entry).unwrap()).unwrap();
        match acquire(wt.path(), "shell") {
            Err(AcquireError::ForeignLock(e)) => assert_eq!(e.pid, other_pid),
            Err(e) => panic!("expected ForeignLock, got {:?}", e),
            Ok(_) => panic!("expected ForeignLock, got Ok"),
        }
    }

    #[test]
    fn foreign_version_lockfile_is_not_cleaned() {
        let wt = make_worktree();
        let path = wt.path().join(".git").join(LOCK_FILENAME);
        // Write raw JSON without a version field → parses as version 0.
        let raw = serde_json::json!({
            "pid": 999_999_999u32,
            "started_at": 0,
            "cmd": "future-gw"
        });
        fs::write(&path, raw.to_string()).unwrap();
        // read_and_clean_stale should return Some(entry) and NOT remove it.
        let entry = read_and_clean_stale(wt.path()).expect("foreign-version entry preserved");
        assert_eq!(entry.version, 0);
        assert!(
            path.exists(),
            "foreign-version lockfile must not be cleaned"
        );
    }

    // unix-only: relies on pid_alive returning false for a fake PID.
    #[cfg(unix)]
    #[test]
    fn cleanup_stale_tmp_files_removes_dead_pids() {
        let wt = make_worktree();
        let git = wt.path().join(".git");
        let dead = git.join(format!("{}.tmp.{}", LOCK_FILENAME, 999_999_999u32));
        fs::write(&dead, "stale").unwrap();
        cleanup_stale_tmp_files(&git);
        assert!(!dead.exists(), "dead-pid tmp file should be removed");
    }
}
