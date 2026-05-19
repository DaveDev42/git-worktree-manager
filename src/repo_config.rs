//! Repo-local `.cwconfig.json` loader.
//!
//! Walks up from `start` looking for the first `.cwconfig.json`. Returns the
//! parsed JSON value alongside the path it was found at. Used by Phase 7.2's
//! layered config resolution; not yet wired into config callers.

use std::path::{Path, PathBuf};

use crate::error::Result;

pub struct RepoConfig {
    pub path: PathBuf,
    pub value: serde_json::Value,
}

/// Maximum directory levels to walk up when searching for `.cwconfig.json`.
/// Prevents unbounded traversal on deep or unusual filesystem layouts.
const MAX_ANCESTOR_DEPTH: usize = 20;

pub fn find_repo_config(start: &Path) -> Option<RepoConfig> {
    let mut cur = start.to_path_buf();
    for _ in 0..MAX_ANCESTOR_DEPTH {
        let candidate = cur.join(".cwconfig.json");
        if candidate.exists() {
            if let Ok(content) = std::fs::read_to_string(&candidate) {
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) {
                    return Some(RepoConfig {
                        path: candidate,
                        value,
                    });
                }
            }
        }
        if !cur.pop() {
            return None;
        }
    }
    None
}

pub fn load_repo_config(start: &Path) -> Result<Option<serde_json::Value>> {
    Ok(find_repo_config(start).map(|c| c.value))
}
