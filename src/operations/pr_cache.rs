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

#[derive(Debug, Deserialize)]
struct GhPr {
    #[serde(rename = "headRefName")]
    head_ref_name: String,
    state: String,
}

/// Run `gh pr list --state all --json headRefName,state --limit N` and parse.
/// Returns None on any failure (gh missing, non-zero exit, JSON parse error).
///
/// Test hooks:
/// - `GW_TEST_GH_JSON` env var: if set, parsed as the `gh` output instead of
///   spawning `gh`.
/// - `GW_TEST_GH_FAIL=1`: simulate a failure.
fn fetch_from_gh(repo: &Path) -> Option<HashMap<String, String>> {
    if std::env::var("GW_TEST_GH_FAIL").ok().as_deref() == Some("1") {
        return None;
    }

    let stdout = if let Ok(json) = std::env::var("GW_TEST_GH_JSON") {
        json
    } else {
        if !crate::git::has_command("gh") {
            return None;
        }
        let limit = GH_FETCH_LIMIT.to_string();
        let result = crate::git::run_command(
            &[
                "gh",
                "pr",
                "list",
                "--state",
                "all",
                "--json",
                "headRefName,state",
                "--limit",
                &limit,
            ],
            Some(repo),
            false,
            true,
        )
        .ok()?;
        if result.returncode != 0 {
            return None;
        }
        result.stdout
    };

    let prs: Vec<GhPr> = serde_json::from_str(stdout.trim()).ok()?;
    let mut map = HashMap::with_capacity(prs.len());
    for pr in prs {
        map.insert(pr.head_ref_name, pr.state);
    }
    Some(map)
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

    #[test]
    fn fetch_parses_gh_json_from_env() {
        std::env::set_var(
            "GW_TEST_GH_JSON",
            r#"[{"headRefName":"feat/foo","state":"OPEN"},{"headRefName":"fix/bar","state":"MERGED"}]"#,
        );
        let prs = fetch_from_gh(std::path::Path::new(".")).expect("parsed");
        std::env::remove_var("GW_TEST_GH_JSON");
        assert_eq!(prs.get("feat/foo").map(String::as_str), Some("OPEN"));
        assert_eq!(prs.get("fix/bar").map(String::as_str), Some("MERGED"));
    }

    #[test]
    fn fetch_returns_none_on_forced_failure() {
        std::env::set_var("GW_TEST_GH_FAIL", "1");
        let result = fetch_from_gh(std::path::Path::new("."));
        std::env::remove_var("GW_TEST_GH_FAIL");
        assert!(result.is_none());
    }
}
