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
