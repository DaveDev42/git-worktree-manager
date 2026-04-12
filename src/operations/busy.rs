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
#[cfg(target_os = "macos")]
use std::process::Command;
use std::sync::OnceLock;

use super::lockfile;

/// Signal source that flagged a process as busy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BusySource {
    Lockfile,
    ProcessScan,
}

/// Information about a single process holding a worktree busy.
#[derive(Debug, Clone)]
pub struct BusyInfo {
    pub pid: u32,
    pub cmd: String,
    /// For lockfile sources, this is the worktree path (the process's
    /// actual cwd is unknown). For process-scan sources, this is the
    /// process's canonicalized cwd.
    pub cwd: PathBuf,
    pub source: BusySource,
}

/// Cached self-process-tree for the lifetime of this `gw` invocation.
static SELF_TREE: OnceLock<HashSet<u32>> = OnceLock::new();

/// Cached raw cwd scan. On unix this is populated once per `gw` invocation
/// (lsof / /proc walk is expensive). Each entry: (pid, cmd, canon_cwd).
static CWD_SCAN_CACHE: OnceLock<Vec<(u32, String, PathBuf)>> = OnceLock::new();

/// Emits the "could not scan processes" warning at most once per process.
/// `gw` is short-lived so this is appropriate; a long-running daemon using
/// this module would need to rework this (currently not a use case).
static SCAN_WARNING: OnceLock<()> = OnceLock::new();

fn compute_self_tree() -> HashSet<u32> {
    let mut tree = HashSet::new();
    tree.insert(std::process::id());

    #[cfg(unix)]
    {
        let mut pid = unsafe { libc::getppid() } as u32;
        for _ in 0..64 {
            // PID 0 is a kernel/orphan marker, not a userland process — skip.
            if pid == 0 {
                break;
            }
            // PID 1 (init/launchd) IS our ancestor when gw was reparented, so
            // exclude it from busy detection just like any other ancestor.
            // Stop walking: init has no meaningful parent for our purposes.
            if pid == 1 {
                tree.insert(pid);
                break;
            }
            tree.insert(pid);
            match parent_of(pid) {
                Some(ppid) if ppid != pid => pid = ppid,
                _ => break,
            }
        }
    }
    tree
}

/// Returns the current process + all ancestor PIDs (via getppid chain).
/// Memoized for the lifetime of the process — the ancestry does not change
/// during a single `gw` invocation.
pub fn self_process_tree() -> &'static HashSet<u32> {
    SELF_TREE.get_or_init(compute_self_tree)
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
#[allow(dead_code)]
fn parent_of(_pid: u32) -> Option<u32> {
    None
}

fn warn_scan_failed(what: &str) {
    if SCAN_WARNING.set(()).is_ok() {
        eprintln!(
            "{} could not scan processes: {}",
            console::style("warning:").yellow(),
            what
        );
    }
}

/// Populate and return the cached cwd scan (all processes, not filtered).
fn cwd_scan() -> &'static [(u32, String, PathBuf)] {
    CWD_SCAN_CACHE.get_or_init(raw_cwd_scan).as_slice()
}

#[cfg(target_os = "linux")]
fn raw_cwd_scan() -> Vec<(u32, String, PathBuf)> {
    let mut out = Vec::new();
    let proc_dir = match std::fs::read_dir("/proc") {
        Ok(d) => d,
        Err(e) => {
            warn_scan_failed(&format!("/proc unreadable: {}", e));
            return out;
        }
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
        // canonicalize so symlinked / bind-mounted cwds match the target.
        // On Linux, readlink on /proc/<pid>/cwd returns " (deleted)" if the
        // process's cwd was unlinked; canonicalize fails and we fall back.
        let cwd_canon = cwd.canonicalize().unwrap_or(cwd.clone());
        let cmd = std::fs::read_to_string(entry.path().join("comm"))
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        out.push((pid, cmd, cwd_canon));
    }
    out
}

#[cfg(target_os = "macos")]
fn raw_cwd_scan() -> Vec<(u32, String, PathBuf)> {
    let mut out = Vec::new();
    // `lsof -a -d cwd -F pcn` prints records of the form:
    //   p<pid>\nc<cmd>\nn<path>\n
    let output = match Command::new("lsof")
        .args(["-a", "-d", "cwd", "-F", "pcn"])
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            warn_scan_failed(&format!("lsof unavailable: {}", e));
            return out;
        }
    };
    if !output.status.success() && output.stdout.is_empty() {
        warn_scan_failed("lsof returned no output");
        return out;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);

    let mut cur_pid: Option<u32> = None;
    let mut cur_cmd = String::new();
    for line in stdout.lines() {
        if let Some(rest) = line.strip_prefix('p') {
            cur_pid = rest.parse().ok();
            cur_cmd.clear();
        } else if let Some(rest) = line.strip_prefix('c') {
            cur_cmd = rest.to_string();
        } else if let Some(rest) = line.strip_prefix('n') {
            if let Some(pid) = cur_pid {
                let cwd = PathBuf::from(rest);
                let cwd_canon = cwd.canonicalize().unwrap_or_else(|_| cwd.clone());
                out.push((pid, cur_cmd.clone(), cwd_canon));
            }
        }
    }
    out
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn raw_cwd_scan() -> Vec<(u32, String, PathBuf)> {
    Vec::new()
}

/// Detect busy processes for a given worktree path.
///
/// Combines the lockfile signal and a process cwd scan. Filters out the
/// current process tree so `gw delete` invoked from within the worktree
/// does not self-report as busy.
///
/// Note: `detect_busy` calls `lockfile::read_and_clean_stale`, which removes
/// lockfiles belonging to dead owners as a self-healing side effect. This
/// means even read-only operations like `gw list` may mutate
/// `<worktree>/.git/gw-session.lock` when a stale file is encountered.
pub fn detect_busy(worktree: &Path) -> Vec<BusyInfo> {
    let exclude = self_process_tree();
    let mut out = Vec::new();

    // Invariant: lockfile entries are pushed before the cwd scan so the
    // dedup check below keeps the lockfile's richer `cmd` (e.g. "claude").
    // Edge case: if the lockfile PID is in self_tree it is skipped entirely,
    // and other PIDs found by the cwd scan are reported with whatever name
    // `/proc/*/comm` or `lsof` provided — not the lockfile's cmd.
    if let Some(entry) = lockfile::read_and_clean_stale(worktree) {
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

fn scan_cwd(worktree: &Path) -> Vec<BusyInfo> {
    let canon_target = match worktree.canonicalize() {
        Ok(p) => p,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for (pid, cmd, cwd) in cwd_scan() {
        // Both sides were canonicalized upstream (handles macOS /var vs
        // /private/var skew). This starts_with is the containment check.
        if cwd.starts_with(&canon_target) {
            out.push(BusyInfo {
                pid: *pid,
                cmd: cmd.clone(),
                cwd: cwd.clone(),
                source: BusySource::ProcessScan,
            });
        }
    }
    out
}

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
        assert!(
            tree.contains(&ppid),
            "expected tree to contain ppid {}",
            ppid
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn scan_cwd_finds_child_with_cwd_in_tempdir() {
        use std::process::Stdio;
        use std::thread::sleep;
        use std::time::{Duration, Instant};

        let dir = tempfile::TempDir::new().unwrap();
        let mut child = Command::new("sleep")
            .arg("30")
            .current_dir(dir.path())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn sleep");

        // Give the OS a beat to register the child's cwd so the first scan
        // usually succeeds; then fall back to polling for slow CI hosts.
        // raw_cwd_scan() bypasses the module-static cache (which may have
        // been populated before the child existed).
        sleep(Duration::from_millis(50));
        let canon = dir
            .path()
            .canonicalize()
            .unwrap_or(dir.path().to_path_buf());
        let matches = |raw: &[(u32, String, std::path::PathBuf)]| -> bool {
            raw.iter()
                .any(|(p, _, cwd)| *p == child.id() && cwd.starts_with(&canon))
        };
        let mut found = matches(&raw_cwd_scan());
        if !found {
            let deadline = Instant::now() + Duration::from_secs(2);
            while Instant::now() < deadline {
                if matches(&raw_cwd_scan()) {
                    found = true;
                    break;
                }
                sleep(Duration::from_millis(50));
            }
        }

        let _ = child.kill();
        let _ = child.wait();

        assert!(
            found,
            "expected to find child pid={} with cwd in {:?}",
            child.id(),
            dir.path()
        );
    }
}
