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
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::process::Command;

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
    pub cwd: PathBuf,
    pub source: BusySource,
}

/// Returns the current process + all ancestor PIDs (via getppid chain).
#[cfg(unix)]
pub fn self_process_tree() -> HashSet<u32> {
    let mut tree = HashSet::new();
    tree.insert(std::process::id());

    let mut pid = unsafe { libc::getppid() } as u32;
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
/// Combines the lockfile signal and a process cwd scan. Filters out the
/// current process tree so `gw delete` invoked from within the worktree
/// does not self-report as busy.
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
                if !cwd_canon.starts_with(&canon_target) {
                    continue;
                }
                out.push(BusyInfo {
                    pid,
                    cmd: cur_cmd.clone(),
                    cwd: cwd_canon,
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
