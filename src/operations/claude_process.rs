//! Live Claude Code process detection — confirms a worktree is actually
//! occupied by a running `claude` instance, not just a stale jsonl event log.
//!
//! Background: [`crate::operations::claude_session`] flags a worktree as
//! "active" when its `~/.claude/projects/<encoded>/*.jsonl` has an event
//! within the activity threshold. That signal alone is too coarse — when a
//! user exits Claude Code cleanly the jsonl tail keeps its recent timestamp
//! and `gw delete` keeps refusing for the full threshold window.
//!
//! This module supplies a complementary signal: scan the live process table
//! for a real `claude` binary (identified by its `txt` mapping under
//! `~/.local/share/claude/versions/` or the macOS `Claude.app` bundle) whose
//! `cwd` is the worktree, or that has an open file under
//! `<worktree>/.claude/`. argv[0] is intentionally not trusted because Claude
//! Code rewrites it to a version string at runtime.
//!
//! The scan is cached in a `OnceLock` for the lifetime of the `gw`
//! invocation, so callers may invoke `has_live_claude_in` repeatedly without
//! re-shelling out to `lsof` / re-walking `/proc`.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// One live process's relevant filesystem state.
#[derive(Debug, Clone)]
pub(crate) struct ProcessSnapshot {
    /// Retained for diagnostics / future filtering, not read by the
    /// current matchers. `#[allow(dead_code)]` keeps the zero-warning
    /// policy without losing the field.
    #[allow(dead_code)]
    pub pid: u32,
    /// Process cwd (canonicalized when possible).
    pub cwd: Option<PathBuf>,
    /// Paths from `txt`-mapped files (executable + dynamic libraries on
    /// macOS; main executable on Linux). Used to identify true `claude`
    /// processes despite argv[0] rewriting.
    pub txt_paths: Vec<PathBuf>,
    /// Open regular-file / directory fds. Filtered to paths under any
    /// `<...>/.claude` to keep the snapshot small.
    pub claude_fd_paths: Vec<PathBuf>,
}

/// Predicate: does this snapshot represent a live `claude` process?
///
/// Matching rule: any `txt`-mapped path contains either
/// `/.local/share/claude/versions/` (npm-style installs) or
/// `/Claude.app/` (macOS desktop app — for completeness; that bundle does
/// not normally open per-worktree fds, but we keep the rule symmetric).
pub(crate) fn is_claude_process(snap: &ProcessSnapshot) -> bool {
    snap.txt_paths.iter().any(|p| {
        let s = p.to_string_lossy();
        s.contains("/.local/share/claude/versions/") || s.contains("/Claude.app/")
    })
}

/// Predicate: is this process holding the given worktree?
///
/// `worktree` must already be canonicalized by the caller (so we don't
/// canonicalize once per process). Matches when:
///   * cwd equals worktree or is a descendant, OR
///   * any `<worktree>/.claude` fd is held open.
pub(crate) fn process_holds_worktree(snap: &ProcessSnapshot, worktree_canon: &Path) -> bool {
    if let Some(cwd) = &snap.cwd {
        if cwd == worktree_canon || cwd.starts_with(worktree_canon) {
            return true;
        }
    }
    let dot_claude = worktree_canon.join(".claude");
    snap.claude_fd_paths
        .iter()
        .any(|p| p == &dot_claude || p.starts_with(&dot_claude))
}

/// Cached process snapshot for the lifetime of this `gw` invocation.
static SNAPSHOT_CACHE: OnceLock<Vec<ProcessSnapshot>> = OnceLock::new();

fn snapshot() -> &'static [ProcessSnapshot] {
    SNAPSHOT_CACHE.get_or_init(scan_processes).as_slice()
}

/// Force-populate the snapshot cache. Intended for parallel prewarm: spawn
/// this on a background thread alongside `busy::prewarm_cwd_scan` so the
/// two `lsof` calls overlap. Subsequent `has_live_claude_in` calls then
/// hit the cache. Safe to call from multiple threads — `OnceLock` ensures
/// the scan runs at most once.
pub(crate) fn prewarm() {
    let _ = snapshot();
}

/// Returns true iff a live `claude` process is occupying `worktree`.
///
/// Returns `false` when the scan could not run (lsof missing, /proc
/// unreadable). The caller should treat that as "no signal available" —
/// preserving the conservative default of relying on the jsonl-only check.
pub fn has_live_claude_in(worktree: &Path) -> bool {
    let canon = worktree
        .canonicalize()
        .unwrap_or_else(|_| worktree.to_path_buf());
    snapshot()
        .iter()
        .any(|s| is_claude_process(s) && process_holds_worktree(s, &canon))
}

#[cfg(target_os = "macos")]
fn scan_processes() -> Vec<ProcessSnapshot> {
    use std::collections::HashMap;
    use std::process::Command;

    // Stage 1: enumerate candidate PIDs via `ps -Ao pid,comm`, filtering on
    // kernel-recorded command name. This is cheap (~50ms) and avoids the
    // heavy system-wide `lsof` whose latency balloons under disk/IO load.
    // `comm` reflects the basename the kernel knows the process by, which
    // is unaffected by Claude Code's argv[0] rewriting — so this 1st-stage
    // filter is sound even before the txt-mapping double-check below.
    let ps_out = match Command::new("ps").args(["-Ao", "pid=,comm="]).output() {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };
    let mut candidate_pids: Vec<u32> = Vec::new();
    for line in String::from_utf8_lossy(&ps_out.stdout).lines() {
        let line = line.trim_start();
        let (pid_s, rest) = match line.split_once(' ') {
            Some(p) => p,
            None => continue,
        };
        let Ok(pid) = pid_s.parse::<u32>() else {
            continue;
        };
        // `ps -o comm=` on macOS prints the full executable path. Take
        // basename and match either `claude` (CLI) or `Claude` (Claude.app).
        let basename = std::path::Path::new(rest.trim())
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        if basename == "claude" || basename == "Claude" {
            candidate_pids.push(pid);
        }
    }
    if candidate_pids.is_empty() {
        return Vec::new();
    }

    // Stage 2: targeted `lsof -p <pid>,<pid>,...` — only the candidates,
    // not every process on the box. Records: p<pid>, f<fd>, n<path>.
    let pid_arg = candidate_pids
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let output = match Command::new("lsof")
        .args(["-F", "pfn", "+c", "0", "-p", &pid_arg])
        .output()
    {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };
    if !output.status.success() && output.stdout.is_empty() {
        return Vec::new();
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut by_pid: HashMap<u32, ProcessSnapshot> = HashMap::new();
    let mut cur_pid: Option<u32> = None;
    let mut cur_fd = String::new();
    for line in stdout.lines() {
        if let Some(rest) = line.strip_prefix('p') {
            cur_pid = rest.parse().ok();
            cur_fd.clear();
        } else if let Some(rest) = line.strip_prefix('f') {
            cur_fd = rest.to_string();
        } else if let Some(rest) = line.strip_prefix('n') {
            let Some(pid) = cur_pid else { continue };
            let path = PathBuf::from(rest);
            let entry = by_pid.entry(pid).or_insert_with(|| ProcessSnapshot {
                pid,
                cwd: None,
                txt_paths: Vec::new(),
                claude_fd_paths: Vec::new(),
            });
            match cur_fd.as_str() {
                "cwd" => {
                    let canon = path.canonicalize().unwrap_or(path);
                    entry.cwd = Some(canon);
                }
                "txt" => entry.txt_paths.push(path),
                _ => {
                    // Keep only fds under some ".claude" directory — these
                    // are the per-worktree settings.json / .claude/ dir
                    // that Claude Code holds open while running.
                    if path.to_string_lossy().contains("/.claude") {
                        let canon = path.canonicalize().unwrap_or(path);
                        entry.claude_fd_paths.push(canon);
                    }
                }
            }
        }
    }
    by_pid.into_values().collect()
}

#[cfg(target_os = "linux")]
fn scan_processes() -> Vec<ProcessSnapshot> {
    let proc_dir = match std::fs::read_dir("/proc") {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for entry in proc_dir.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let pid: u32 = match name.parse() {
            Ok(n) => n,
            Err(_) => continue,
        };
        let proc_path = entry.path();

        // 1st-stage filter: kernel-recorded comm. Cheap read on every
        // process; skips the expensive fd directory walk for non-claude
        // processes (the box typically has hundreds of those).
        // /proc/<pid>/comm is truncated to 15 chars but "claude" / "Claude"
        // both fit comfortably.
        let comm = match std::fs::read_to_string(proc_path.join("comm")) {
            Ok(s) => s.trim().to_string(),
            Err(_) => continue,
        };
        if comm != "claude" && comm != "Claude" {
            continue;
        }

        let cwd = std::fs::read_link(proc_path.join("cwd"))
            .ok()
            .map(|p| p.canonicalize().unwrap_or(p));
        // exe is the main executable (Linux equivalent of macOS lsof's txt
        // record). Library mappings live in /proc/<pid>/maps; we don't need
        // them — exe alone suffices to identify a `claude` binary.
        let mut txt_paths = Vec::new();
        if let Ok(exe) = std::fs::read_link(proc_path.join("exe")) {
            txt_paths.push(exe);
        }
        let mut claude_fd_paths = Vec::new();
        if let Ok(fds) = std::fs::read_dir(proc_path.join("fd")) {
            for fd_entry in fds.flatten() {
                if let Ok(target) = std::fs::read_link(fd_entry.path()) {
                    if target.to_string_lossy().contains("/.claude") {
                        let canon = target.canonicalize().unwrap_or(target);
                        claude_fd_paths.push(canon);
                    }
                }
            }
        }
        out.push(ProcessSnapshot {
            pid,
            cwd,
            txt_paths,
            claude_fd_paths,
        });
    }
    out
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn scan_processes() -> Vec<ProcessSnapshot> {
    // No portable way to enumerate process cwds + open fds on Windows
    // without extra deps. Returning empty disables the live-process check —
    // behavior falls back to the jsonl-only signal, which matches current
    // (pre-this-module) behavior on Windows.
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(pid: u32, cwd: Option<&str>, txt: &[&str], fds: &[&str]) -> ProcessSnapshot {
        ProcessSnapshot {
            pid,
            cwd: cwd.map(PathBuf::from),
            txt_paths: txt.iter().map(PathBuf::from).collect(),
            claude_fd_paths: fds.iter().map(PathBuf::from).collect(),
        }
    }

    #[test]
    fn is_claude_process_detects_user_install() {
        let s = snap(
            1,
            Some("/wt"),
            &["/Users/dave/.local/share/claude/versions/2.1.121"],
            &[],
        );
        assert!(is_claude_process(&s));
    }

    #[test]
    fn is_claude_process_detects_macos_desktop_app() {
        let s = snap(
            1,
            Some("/wt"),
            &["/Applications/Claude.app/Contents/MacOS/Claude"],
            &[],
        );
        assert!(is_claude_process(&s));
    }

    #[test]
    fn is_claude_process_rejects_unrelated_binary() {
        let s = snap(1, Some("/wt"), &["/usr/bin/zsh"], &[]);
        assert!(!is_claude_process(&s));
    }

    #[test]
    fn is_claude_process_rejects_substring_outside_claude_path() {
        // Defensive: a `claude` substring outside the recognized install
        // patterns must not match. (`/etc/claude/foo` is not a Claude Code
        // installation.)
        let s = snap(1, Some("/wt"), &["/etc/claude/foo"], &[]);
        assert!(!is_claude_process(&s));
    }

    #[test]
    fn process_holds_worktree_matches_exact_cwd() {
        let s = snap(1, Some("/wt"), &[], &[]);
        assert!(process_holds_worktree(&s, Path::new("/wt")));
    }

    #[test]
    fn process_holds_worktree_matches_descendant_cwd() {
        let s = snap(1, Some("/wt/subdir/inner"), &[], &[]);
        assert!(process_holds_worktree(&s, Path::new("/wt")));
    }

    #[test]
    fn process_holds_worktree_matches_dot_claude_fd() {
        let s = snap(1, Some("/elsewhere"), &[], &["/wt/.claude/settings.json"]);
        assert!(process_holds_worktree(&s, Path::new("/wt")));
    }

    #[test]
    fn process_holds_worktree_rejects_unrelated_cwd_and_fds() {
        let s = snap(
            1,
            Some("/elsewhere"),
            &[],
            &["/Users/dave/.claude/settings.json"],
        );
        assert!(!process_holds_worktree(&s, Path::new("/wt")));
    }

    #[test]
    fn process_holds_worktree_rejects_sibling_with_shared_prefix() {
        // /wt-other should not match /wt — Path::starts_with is component-
        // wise so this is already safe, but pin it down with a test so a
        // future refactor to string-prefix matching gets caught.
        let s = snap(1, Some("/wt-other/sub"), &[], &[]);
        assert!(!process_holds_worktree(&s, Path::new("/wt")));
    }
}
