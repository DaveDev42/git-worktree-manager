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

use super::{claude_process, claude_session, lockfile};
use chrono::Duration as ChronoDuration;

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

/// Cached self-process-tree for the lifetime of this `gw` invocation.
static SELF_TREE: OnceLock<HashSet<u32>> = OnceLock::new();

/// Cached sibling set — processes sharing `gw`'s direct parent PID, captured
/// once per invocation. This covers shell pipeline co-members (e.g. when a
/// user runs `gw list | head` the `head` process is gw's sibling, not an
/// ancestor) and a few other co-spawned helpers.
static SELF_SIBLINGS: OnceLock<HashSet<u32>> = OnceLock::new();

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

/// Compute the set of processes sharing `gw`'s process group ID.
///
/// Shells set up pipelines (`gw list | head | awk`) by putting all members
/// in a single process group that becomes the foreground job. Using pgid
/// as the sibling criterion matches exactly those pipeline co-members and
/// excludes them from busy detection — they inherited the shell's cwd but
/// are transient artifacts of the current command, not real occupants.
///
/// This is deliberately narrower than "processes sharing our ppid": the
/// broader criterion would also exclude legitimate busy processes that
/// happen to be spawned by the same parent as `gw` (e.g. a test harness
/// running both a long-lived worker and `gw` from the same Cargo runner).
#[cfg(unix)]
fn compute_self_siblings() -> HashSet<u32> {
    let mut siblings = HashSet::new();
    let our_pid = std::process::id();
    let our_pgid = unsafe { libc::getpgrp() } as u32;
    if our_pgid == 0 || our_pgid == 1 {
        return siblings;
    }
    // Distinguish two scenarios with the same raw pgid test:
    //   (a) gw is a member of a shell pipeline (`gw list | head`). The shell
    //       placed the pipeline in its own process group, so our pgid differs
    //       from our parent's pgid. Pipeline co-members share our pgid and
    //       are safe to exclude.
    //   (b) gw was spawned by a non-shell parent that did not call setpgid
    //       (e.g. `cargo test` spawning both gw and a long-lived worker).
    //       Our pgid equals our parent's pgid, which means "same pgid" also
    //       matches unrelated siblings that legitimately occupy a worktree.
    //       In this case we return an empty set and let the ancestor-only
    //       filter handle things.
    let parent_pid = unsafe { libc::getppid() } as u32;
    if parent_pid == 0 {
        return siblings;
    }
    let parent_pgid = pgid_of(parent_pid).unwrap_or(0);
    if parent_pgid == our_pgid {
        return siblings;
    }
    for (pid, _, _) in cwd_scan() {
        if *pid == our_pid {
            continue;
        }
        if let Some(pgid) = pgid_of(*pid) {
            if pgid == our_pgid {
                siblings.insert(*pid);
            }
        }
    }
    siblings
}

#[cfg(not(unix))]
fn compute_self_siblings() -> HashSet<u32> {
    HashSet::new()
}

#[cfg(target_os = "linux")]
fn pgid_of(pid: u32) -> Option<u32> {
    let status = std::fs::read_to_string(format!("/proc/{}/stat", pid)).ok()?;
    // /proc/<pid>/stat: "pid (comm) state ppid pgid ..."
    // Parse from the last ')' to avoid confusion with spaces/parens in comm.
    let after_comm = status.rsplit_once(')')?.1;
    let fields: Vec<&str> = after_comm.split_whitespace().collect();
    // After ')' the fields are: state ppid pgid ...
    // So pgid is index 2.
    fields.get(2)?.parse().ok()
}

#[cfg(target_os = "macos")]
fn pgid_of(pid: u32) -> Option<u32> {
    let out = Command::new("ps")
        .args(["-o", "pgid=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
#[allow(dead_code)]
fn pgid_of(_pid: u32) -> Option<u32> {
    None
}

/// Returns the memoized sibling set (see `compute_self_siblings`).
pub fn self_siblings() -> &'static HashSet<u32> {
    SELF_SIBLINGS.get_or_init(compute_self_siblings)
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

#[allow(dead_code)]
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

/// Force-populate the cwd scan cache. Intended for parallel prewarm so the
/// system-wide `lsof` runs concurrently with `claude_process::prewarm`.
/// Safe to call from multiple threads — `OnceLock` ensures the scan runs
/// at most once.
pub(crate) fn prewarm_cwd_scan() {
    let _ = cwd_scan();
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

/// Heuristic: does a cmd string look like an argv[0] that was overwritten
/// with a version or status string rather than a program name? Example from
/// the wild: Claude Code rewrites argv[0] to "2.1.104". `lsof` reports argv[0]
/// for macOS processes, so these junk values bleed into busy reporting.
/// We detect the pattern (all digits, dots, and optional leading `v`) and
/// fall back to a `ps -o comm=` lookup, which returns the kernel-recorded
/// basename.
///
/// Linux's `/proc/<pid>/comm` already reports the kernel-recorded name so
/// this heuristic is only used on macOS; the tests remain cross-platform.
#[cfg_attr(not(any(target_os = "macos", test)), allow(dead_code))]
fn is_suspicious_cmd(cmd: &str) -> bool {
    if cmd.is_empty() {
        return true;
    }
    let mut chars = cmd.chars();
    let first = chars.next().unwrap();
    let starts_ok = first == 'v' || first.is_ascii_digit();
    if !starts_ok {
        return false;
    }
    let mut seen_digit = first.is_ascii_digit();
    for c in chars {
        if c.is_ascii_digit() {
            seen_digit = true;
        } else if c != '.' {
            return false;
        }
    }
    seen_digit
}

#[cfg(target_os = "macos")]
fn kernel_comm(pid: u32) -> Option<String> {
    let out = Command::new("ps")
        .args(["-o", "comm=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let raw = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if raw.is_empty() {
        return None;
    }
    // `ps -o comm=` on macOS returns the full executable path. Take basename.
    let base = std::path::Path::new(&raw)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or(raw);
    Some(base)
}

#[cfg(target_os = "macos")]
fn raw_cwd_scan() -> Vec<(u32, String, PathBuf)> {
    let mut out = Vec::new();
    // `lsof -a -d cwd -F pcn` prints records of the form:
    //   p<pid>\nc<cmd>\nn<path>\n
    // `+c 0` disables lsof's default 9-char COMMAND truncation so multi-word
    // names like "tmux: server" survive intact for the multiplexer filter.
    let output = match Command::new("lsof")
        .args(["-a", "-d", "cwd", "-F", "pcn", "+c", "0"])
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
                let cmd = if is_suspicious_cmd(&cur_cmd) {
                    kernel_comm(pid).unwrap_or_else(|| cur_cmd.clone())
                } else {
                    cur_cmd.clone()
                };
                out.push((pid, cmd, cwd_canon));
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
    let exclude_tree = self_process_tree();
    let exclude_siblings = self_siblings();
    let is_excluded = |pid: u32| exclude_tree.contains(&pid) || exclude_siblings.contains(&pid);
    let mut out = Vec::new();

    // Invariant: lockfile entries are pushed before the cwd scan so the
    // dedup check below keeps the lockfile's richer `cmd` (e.g. "claude").
    // Edge case: if the lockfile PID is in self_tree/self_siblings it is
    // skipped entirely, and other PIDs found by the cwd scan are reported
    // with whatever name `/proc/*/comm` or `lsof` provided — not the
    // lockfile's cmd.
    if let Some(entry) = lockfile::read_and_clean_stale(worktree) {
        if !is_excluded(entry.pid) {
            out.push(BusyInfo {
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

    for info in scan_cwd(worktree) {
        if is_excluded(info.pid) {
            continue;
        }
        if out.iter().any(|b| b.pid == info.pid) {
            continue;
        }
        out.push(info);
    }

    out
}

/// Fast busy detection using only the session lockfile.
///
/// Unlike [`detect_busy`], this does not perform a system-wide process cwd
/// scan (lsof on macOS, /proc walk on Linux). The cwd scan takes ~1.5s on
/// typical macOS systems and dominates `gw list` latency, so read-only
/// display paths use this variant.
///
/// This trades coverage for speed: worktrees entered via external `cd`
/// without a `gw shell`/`gw start` session will not be flagged as busy.
/// Commands that need strong busy guarantees (`gw delete`) continue to
/// use [`detect_busy`].
///
/// Like [`detect_busy`], this calls [`lockfile::read_and_clean_stale`]
/// and may silently remove a stale `<worktree>/.git/gw-session.lock` as
/// a self-healing side effect. `gw list` (the primary caller) therefore
/// mutates lockfiles on every invocation, even though it is nominally
/// read-only.
pub fn detect_busy_lockfile_only(worktree: &Path) -> Vec<BusyInfo> {
    // Skip self_siblings: it internally triggers cwd_scan (lsof / /proc walk)
    // which is exactly what this fast path exists to avoid. Pipeline co-members
    // of this gw invocation are short-lived CLI tools (e.g. `gw list | head`)
    // that never call `gw shell`/`gw start`, so they cannot own a lockfile.
    // Ancestor-only exclusion is sufficient in practice — and in the rare case
    // where a true sibling (e.g. a backgrounded `gw start`) does own a
    // lockfile, reporting its worktree as busy is correct, not a false positive.
    let exclude_tree = self_process_tree();
    let is_excluded = |pid: u32| exclude_tree.contains(&pid);
    let mut out = Vec::new();

    if let Some(entry) = lockfile::read_and_clean_stale(worktree) {
        if !is_excluded(entry.pid) {
            out.push(BusyInfo {
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

    out
}

/// Threshold for considering a Claude jsonl event "active." Spec value.
const CLAUDE_ACTIVITY_THRESHOLD_MIN: i64 = 10;

/// The two-stage "Claude is here" gate. Returns the list of active
/// sessions iff (a) the jsonl tail has an event within the threshold AND
/// (b) a live `claude` process is occupying `worktree` (cwd or `.claude`
/// fd). Returns `None` when either gate fails or no project dir is found.
///
/// This is the single source of truth for the gate — `detect_busy_tiered`
/// uses it for the full hard/soft dispatch in `gw delete`, and
/// `display::get_worktree_status` uses it as the "busy" check for read-
/// only surfaces (`gw status` / `gw list`).
pub fn active_claude_sessions(worktree: &Path) -> Option<Vec<claude_session::ActiveSession>> {
    let proj_dir = claude_session::project_dir_for(worktree)?;
    let threshold = ChronoDuration::minutes(CLAUDE_ACTIVITY_THRESHOLD_MIN);
    let sessions = claude_session::find_active_sessions(&proj_dir, worktree, threshold);
    if sessions.is_empty() || !claude_process::has_live_claude_in(worktree) {
        return None;
    }
    Some(sessions)
}

/// Tiered busy detection: returns `(hard, soft)` separately so the caller
/// can render distinct refusal messages.
///
/// Hard signals (refuse strongly, override = `--force`):
///   * Active Claude Code session: jsonl event within threshold AND a live
///     `claude` process is occupying the worktree (cwd or `.claude` fd).
///   * Explicit lockfile
///
/// Soft signals (refuse with a warning, same `--force` override):
///   * Process cwd scan results that are not already represented by a
///     Hard signal (deduped by PID; PID 0 sentinels for ClaudeSession
///     are not deduped).
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

    // Hard: active Claude sessions. The two-stage gate (jsonl event AND
    // live `claude` process) lives in `active_claude_sessions` so that
    // read-only surfaces (`gw status` / `gw list`) share the same check.
    if let Some(sessions) = active_claude_sessions(worktree) {
        for s in sessions {
            // session_id is a UUID; surface as cmd "claude (session <id>)" with
            // PID 0 as a sentinel meaning "not a process PID, informational entry".
            let secs_ago = (chrono::Utc::now() - s.last_activity).num_seconds().max(0) as u64;
            hard.push(BusyInfo {
                pid: 0,
                cmd: format!("claude (session {})", s.session_id),
                cwd: worktree.to_path_buf(),
                source: BusySource::ClaudeSession,
                tier: BusyTier::Hard,
                tty: None,
                started_secs_ago: Some(secs_ago),
            });
        }
    }

    // Soft: process cwd scan, deduped against PIDs already in Hard (PID 0
    // sentinels for ClaudeSession do not participate in dedup since real
    // processes have non-zero PIDs).
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

/// Terminal multiplexers whose server process may have been launched from
/// within a worktree but does not meaningfully "occupy" it — the real work
/// happens in child shells / tools, which the cwd scan reports independently.
/// Reporting the multiplexer itself just produces noise when running
/// `gw delete` from a pane hosted by that multiplexer.
///
/// Matched against `/proc/<pid>/comm` on Linux (≤15 chars; may reflect
/// `prctl(PR_SET_NAME)` rather than argv[0], e.g. "tmux: server") or `lsof`'s
/// COMMAND field on macOS (we pass `+c 0` to disable its default 9-char
/// truncation — see `raw_cwd_scan`). GNU screen's detached server renames
/// itself to uppercase "SCREEN" via prctl, so both cases are listed.
fn is_multiplexer(cmd: &str) -> bool {
    matches!(
        cmd,
        "zellij" | "tmux" | "tmux: server" | "tmate" | "tmate: server" | "screen" | "SCREEN"
    )
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
            if is_multiplexer(cmd) {
                continue;
            }
            out.push(BusyInfo {
                pid: *pid,
                cmd: cmd.clone(),
                cwd: cwd.clone(),
                source: BusySource::ProcessScan,
                tier: BusyTier::Soft,
                tty: None,
                started_secs_ago: None,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_suspicious_cmd_flags_version_strings() {
        assert!(is_suspicious_cmd(""));
        assert!(is_suspicious_cmd("2.1.104"));
        assert!(is_suspicious_cmd("0.0.1"));
        assert!(is_suspicious_cmd("v1.2.3"));
        assert!(is_suspicious_cmd("42"));
    }

    #[test]
    fn is_suspicious_cmd_accepts_real_names() {
        assert!(!is_suspicious_cmd("claude"));
        assert!(!is_suspicious_cmd("node"));
        assert!(!is_suspicious_cmd("zsh"));
        assert!(!is_suspicious_cmd("tmux: server"));
        assert!(!is_suspicious_cmd("python3"));
        assert!(!is_suspicious_cmd("v"));
        assert!(!is_suspicious_cmd("vim"));
    }

    #[test]
    fn is_multiplexer_matches_known_names() {
        for name in [
            "zellij",
            "tmux",
            "tmux: server",
            "tmate",
            "tmate: server",
            "screen",
            "SCREEN",
        ] {
            assert!(is_multiplexer(name), "expected match for {:?}", name);
        }
    }

    #[test]
    fn is_multiplexer_rejects_non_multiplexers() {
        for name in [
            "",
            "zsh",
            "bash",
            "claude",
            "tmuxinator",
            "ztmux",
            "zellij-server",
            "Screen",
        ] {
            assert!(!is_multiplexer(name), "expected no match for {:?}", name);
        }
    }

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

    #[test]
    fn detect_busy_tiered_returns_hard_for_lockfile() {
        use std::process::{Command, Stdio};
        let dir = tempfile::tempdir().unwrap();
        // Mark a fake .git dir so lock_path resolves predictably.
        let git_dir = dir.path().join(".git");
        std::fs::create_dir_all(&git_dir).unwrap();
        // Spawn a short-lived child process to get a live PID that is NOT in
        // self_process_tree (which excludes all ancestors up to init, but NOT
        // descendants). Use sleep so the child stays alive through the assertion.
        let mut child = Command::new("sleep")
            .arg("30")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn sleep");
        let child_pid = child.id();
        // Write the lockfile directly with the child's PID so it is live and
        // not excluded by the ancestor-chain filter.
        let entry = crate::operations::lockfile::LockEntry {
            version: crate::operations::lockfile::LOCK_VERSION,
            pid: child_pid,
            started_at: 0,
            cmd: "claude".to_string(),
        };
        std::fs::write(
            git_dir.join("gw-session.lock"),
            serde_json::to_string(&entry).unwrap(),
        )
        .unwrap();
        let (hard, _soft) = detect_busy_tiered(dir.path());
        let _ = child.kill();
        let _ = child.wait();
        assert!(hard
            .iter()
            .any(|b| matches!(b.source, BusySource::Lockfile)));
        assert!(hard.iter().all(|b| matches!(b.tier, BusyTier::Hard)));
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn scan_cwd_finds_child_with_cwd_in_tempdir() {
        use std::process::{Command, Stdio};
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

    /// Regression: a stale jsonl with a recent timestamp but no live
    /// `claude` process owning the worktree must NOT produce a Hard
    /// ClaudeSession signal. This is the "user just exited Claude
    /// cleanly, then ran cw delete" scenario from the bug report.
    ///
    /// We exercise it by pointing `$HOME` at a tempdir, planting a
    /// realistic jsonl under `~/.claude/projects/<encoded>/`, and
    /// confirming the test process (which does not look like a Claude
    /// install via its txt mappings) does not satisfy the live-process
    /// gate.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn detect_busy_tiered_no_hard_when_jsonl_active_but_no_live_claude() {
        use crate::operations::test_env::{env_lock, EnvGuard};
        let _lock = env_lock();
        let _guard = EnvGuard::capture(&["HOME"]);

        let home = tempfile::tempdir().unwrap();
        std::env::set_var("HOME", home.path());

        let wt = tempfile::tempdir().unwrap();
        let wt_canon = wt.path().canonicalize().unwrap_or(wt.path().to_path_buf());

        // Encode the worktree path the way Claude Code does (see
        // claude_session::encode_project_dir).
        let encoded = wt_canon.to_string_lossy().replace(['/', '.'], "-");
        let proj_dir = home.path().join(".claude").join("projects").join(encoded);
        std::fs::create_dir_all(&proj_dir).unwrap();

        // Plant a jsonl whose newest event is "now" — i.e. well within the
        // 10-minute activity threshold — and whose `cwd` matches the
        // worktree (otherwise find_active_sessions filters it out).
        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let line = serde_json::json!({
            "timestamp": now,
            "cwd": wt_canon.to_string_lossy(),
        });
        std::fs::write(
            proj_dir.join("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee.jsonl"),
            format!("{}\n", line),
        )
        .unwrap();

        let (hard, _soft) = detect_busy_tiered(wt.path());
        assert!(
            !hard
                .iter()
                .any(|b| matches!(b.source, BusySource::ClaudeSession)),
            "expected no Hard ClaudeSession when no live claude holds the worktree, got: {:?}",
            hard
        );
    }
}
