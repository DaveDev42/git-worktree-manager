//! Hard-tier in-use signal: detects active Claude Code sessions in a
//! worktree by inspecting `~/.claude/projects/<encoded>/*.jsonl` event tails.
//!
//! Encoding rule mirrors Claude Code's own: replace `/` and `.` with `-`,
//! drop trailing slash. Verified empirically against `~/.claude/projects/`
//! contents during design.

use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};

/// Encode an absolute filesystem path to the directory name Claude Code
/// uses under `~/.claude/projects/`. `/` and `.` become `-`. Trailing
/// path separators are trimmed.
pub fn encode_project_dir(path: &Path) -> String {
    let s = path.to_string_lossy();
    let trimmed = s.trim_end_matches('/');
    trimmed.replace(['/', '.'], "-")
}

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

/// Companion: extract the `cwd` field from the same newest event. Used in
/// Task 4 for path-encoding-collision defense. Returns `None` if not present.
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
