use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::constants::{sanitize_branch_name, CLAUDE_SESSION_PREFIX_LENGTH};
use crate::error::Result;
use crate::git::normalize_branch_name;

/// Session metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub branch: String,
    pub ai_tool: String,
    pub worktree_path: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Get the base sessions directory.
pub fn get_sessions_dir() -> PathBuf {
    let dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config")
        .join("claude-worktree")
        .join("sessions");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Get the session directory for a specific branch.
pub fn get_session_dir(branch_name: &str) -> PathBuf {
    let branch = normalize_branch_name(branch_name);
    let safe = sanitize_branch_name(branch);
    let dir = get_sessions_dir().join(safe);
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Check if a Claude native session exists for the given worktree path.
pub fn claude_native_session_exists(worktree_path: &Path) -> bool {
    let path_str = worktree_path
        .canonicalize()
        .unwrap_or_else(|_| worktree_path.to_path_buf())
        .to_string_lossy()
        .to_string();

    // Encode path: replace non-alphanumeric with hyphens
    let encoded: String = path_str
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();

    let claude_projects_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".claude")
        .join("projects");

    if !claude_projects_dir.exists() {
        return false;
    }

    // Direct match
    if encoded.len() <= 255 {
        let project_dir = claude_projects_dir.join(&encoded);
        if project_dir.is_dir() && has_jsonl_files(&project_dir) {
            return true;
        }
    }

    // Prefix matching for long paths
    if encoded.len() > CLAUDE_SESSION_PREFIX_LENGTH {
        let prefix = &encoded[..CLAUDE_SESSION_PREFIX_LENGTH];
        if let Ok(entries) = std::fs::read_dir(&claude_projects_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if entry.path().is_dir()
                    && name_str.starts_with(prefix)
                    && has_jsonl_files(&entry.path())
                {
                    return true;
                }
            }
        }
    }

    false
}

fn has_jsonl_files(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .any(|e| e.path().extension().map(|ext| ext == "jsonl").unwrap_or(false))
        })
        .unwrap_or(false)
}

/// Save session metadata for a branch.
pub fn save_session_metadata(
    branch_name: &str,
    ai_tool: &str,
    worktree_path: &str,
) -> Result<()> {
    let session_dir = get_session_dir(branch_name);
    let metadata_file = session_dir.join("metadata.json");

    let now = chrono_now_iso();

    let mut metadata = SessionMetadata {
        branch: branch_name.to_string(),
        ai_tool: ai_tool.to_string(),
        worktree_path: worktree_path.to_string(),
        created_at: now.clone(),
        updated_at: now,
    };

    // Preserve created_at if metadata already exists
    if metadata_file.exists() {
        if let Ok(content) = std::fs::read_to_string(&metadata_file) {
            if let Ok(existing) = serde_json::from_str::<SessionMetadata>(&content) {
                metadata.created_at = existing.created_at;
            }
        }
    }

    let content = serde_json::to_string_pretty(&metadata)?;
    std::fs::write(&metadata_file, content)?;
    Ok(())
}

/// Load session metadata for a branch.
pub fn load_session_metadata(branch_name: &str) -> Option<SessionMetadata> {
    let session_dir = get_session_dir(branch_name);
    let metadata_file = session_dir.join("metadata.json");

    if !metadata_file.exists() {
        return None;
    }

    let content = std::fs::read_to_string(&metadata_file).ok()?;
    serde_json::from_str(&content).ok()
}

/// Delete all session data for a branch.
pub fn delete_session(branch_name: &str) {
    let session_dir = get_session_dir(branch_name);
    if session_dir.exists() {
        let _ = std::fs::remove_dir_all(session_dir);
    }
}

/// List all saved sessions.
pub fn list_sessions() -> Vec<SessionMetadata> {
    let sessions_dir = get_sessions_dir();
    let mut sessions = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&sessions_dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                let metadata_file = entry.path().join("metadata.json");
                if metadata_file.exists() {
                    if let Ok(content) = std::fs::read_to_string(&metadata_file) {
                        if let Ok(meta) = serde_json::from_str::<SessionMetadata>(&content) {
                            sessions.push(meta);
                        }
                    }
                }
            }
        }
    }

    sessions
}

/// Save context information for a branch.
pub fn save_context(branch_name: &str, context: &str) -> Result<()> {
    let session_dir = get_session_dir(branch_name);
    let context_file = session_dir.join("context.txt");
    std::fs::write(&context_file, context)?;
    Ok(())
}

/// Load context information for a branch.
pub fn load_context(branch_name: &str) -> Option<String> {
    let session_dir = get_session_dir(branch_name);
    let context_file = session_dir.join("context.txt");
    if !context_file.exists() {
        return None;
    }
    std::fs::read_to_string(&context_file).ok()
}

/// Public accessor for the ISO timestamp function.
pub fn chrono_now_iso_pub() -> String {
    chrono_now_iso()
}

/// Simple ISO timestamp without chrono dependency.
fn chrono_now_iso() -> String {
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    // Rough ISO format — sufficient for metadata
    let secs = now.as_secs();
    // Convert to rough UTC datetime
    let days = secs / 86400;
    let time_secs = secs % 86400;
    let hours = time_secs / 3600;
    let minutes = (time_secs % 3600) / 60;
    let seconds = time_secs % 60;

    // Approximate date calculation (good enough for metadata)
    let mut year = 1970u64;
    let mut remaining_days = days;
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }
    let mut month = 1u64;
    let month_days = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    for &md in &month_days {
        if remaining_days < md {
            break;
        }
        remaining_days -= md;
        month += 1;
    }
    let day = remaining_days + 1;

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hours, minutes, seconds
    )
}

fn is_leap_year(year: u64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}
