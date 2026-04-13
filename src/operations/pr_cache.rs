//! Batched PR-status cache for `gw list`.
//!
//! Queries `gh pr list` once per invocation (not once per worktree) and
//! persists the result under `~/.cache/gw/pr-status-<repo-hash>.json` with
//! a 60-second TTL. On any failure (gh missing, disk error, corrupt file)
//! returns an empty cache so the caller's fallback path still runs.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const CACHE_TTL_SECS: u64 = 60;
const GH_FETCH_LIMIT: u32 = 500;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheFile {
    fetched_at: u64,
    repo: String,
    prs: HashMap<String, String>,
}

#[derive(Debug, Default, Clone)]
pub struct PrCache {
    map: HashMap<String, String>,
}

impl PrCache {
    pub fn state(&self, branch: &str) -> Option<&str> {
        self.map.get(branch).map(|s| s.as_str())
    }
}

/// Compute a stable short hash for a repository path.
/// Canonicalizes so `/foo/../foo` hashes the same as `/foo`.
fn repo_hash(repo: &Path) -> String {
    let canon = repo.canonicalize().unwrap_or_else(|_| repo.to_path_buf());
    let mut hasher = Sha256::new();
    hasher.update(canon.to_string_lossy().as_bytes());
    let digest = hasher.finalize();
    hex_short(&digest[..8])
}

fn hex_short(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

/// Return the on-disk cache path for a given repo.
/// Returns None if we cannot determine a cache directory on this platform.
fn cache_path_for(repo: &Path) -> Option<PathBuf> {
    let base = dirs::cache_dir()?.join("gw");
    Some(base.join(format!("pr-status-{}.json", repo_hash(repo))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn repo_hash_is_stable_and_short() {
        let p = PathBuf::from("/tmp/some-repo-that-does-not-exist-xyz");
        let h1 = repo_hash(&p);
        let h2 = repo_hash(&p);
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 16);
    }

    #[test]
    fn repo_hash_differs_per_path() {
        let a = repo_hash(&PathBuf::from("/tmp/repo-a-xyz"));
        let b = repo_hash(&PathBuf::from("/tmp/repo-b-xyz"));
        assert_ne!(a, b);
    }

    #[test]
    fn cache_path_contains_repo_hash() {
        let p = PathBuf::from("/tmp/repo-xyz");
        let cp = cache_path_for(&p).expect("cache dir available");
        let s = cp.to_string_lossy();
        assert!(s.contains("gw"));
        assert!(s.contains("pr-status-"));
        assert!(s.ends_with(".json"));
    }
}
