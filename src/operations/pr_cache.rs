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

    /// Load from disk if fresh (and `no_cache` is false), else fetch via
    /// `gh pr list` and persist. Returns an empty cache on any failure so
    /// the caller's fallback path still works.
    pub fn load_or_fetch(repo: &Path, no_cache: bool) -> Self {
        if !no_cache {
            if let Some(map) = load_from_disk(repo) {
                return PrCache { map };
            }
        }
        match fetch_from_gh(repo) {
            Some(map) => {
                write_to_disk(repo, &map);
                PrCache { map }
            }
            None => PrCache::default(),
        }
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
///
/// When `GW_TEST_CACHE_DIR` is set (test hook), uses that path as the base
/// instead of `dirs::cache_dir()` so tests never touch `~/Library/Caches` on
/// macOS (which ignores `XDG_CACHE_HOME`).
fn cache_path_for(repo: &Path) -> Option<PathBuf> {
    let base = if let Ok(dir) = std::env::var("GW_TEST_CACHE_DIR") {
        PathBuf::from(dir).join("gw")
    } else {
        dirs::cache_dir()?.join("gw")
    };
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

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Read cache file if it exists and is still within TTL. Any error → None.
fn load_from_disk(repo: &Path) -> Option<HashMap<String, String>> {
    let path = cache_path_for(repo)?;
    let data = std::fs::read_to_string(&path).ok()?;
    let file: CacheFile = serde_json::from_str(&data).ok()?;
    let age = now_secs().saturating_sub(file.fetched_at);
    if age > CACHE_TTL_SECS {
        return None;
    }
    Some(file.prs)
}

/// Best-effort write. Failures are silently ignored — the in-memory result is
/// still returned to the caller.
fn write_to_disk(repo: &Path, prs: &HashMap<String, String>) {
    let Some(path) = cache_path_for(repo) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let file = CacheFile {
        fetched_at: now_secs(),
        repo: repo.to_string_lossy().into_owned(),
        prs: prs.clone(),
    };
    let Ok(json) = serde_json::to_string(&file) else {
        return;
    };

    // Atomic write: write to <path>.tmp.<pid>, then rename.
    // Concurrent gw runs won't observe a torn JSON file.
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    if std::fs::write(&tmp, json).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::{Mutex, MutexGuard};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// Acquire the env-var serialization lock. Must be held for the entire
    /// duration of any test that mutates GW_TEST_GH_*, XDG_CACHE_HOME, or
    /// other process-global env vars. Recover from poisoning so one failing
    /// test doesn't break the rest.
    fn env_lock() -> MutexGuard<'static, ()> {
        ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

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
        let _g = env_lock();
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
        let _g = env_lock();
        std::env::set_var("GW_TEST_GH_FAIL", "1");
        let result = fetch_from_gh(std::path::Path::new("."));
        std::env::remove_var("GW_TEST_GH_FAIL");
        assert!(result.is_none());
    }

    use tempfile::tempdir;

    fn with_cache_dir<F: FnOnce()>(dir: &std::path::Path, f: F) {
        let prev = std::env::var_os("GW_TEST_CACHE_DIR");
        std::env::set_var("GW_TEST_CACHE_DIR", dir);
        f();
        match prev {
            Some(v) => std::env::set_var("GW_TEST_CACHE_DIR", v),
            None => std::env::remove_var("GW_TEST_CACHE_DIR"),
        }
    }

    #[test]
    fn load_from_disk_returns_fresh_entry() {
        let _g = env_lock();
        let dir = tempdir().unwrap();
        with_cache_dir(dir.path(), || {
            let repo = std::path::Path::new("/tmp/repo-xyz");
            let path = cache_path_for(repo).unwrap();
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
            let file = CacheFile {
                fetched_at: now,
                repo: repo.to_string_lossy().into_owned(),
                prs: [("feat/a".to_string(), "OPEN".to_string())].into_iter().collect(),
            };
            std::fs::write(&path, serde_json::to_string(&file).unwrap()).unwrap();

            let loaded = load_from_disk(repo).expect("fresh cache");
            assert_eq!(loaded.get("feat/a").map(String::as_str), Some("OPEN"));
        });
    }

    #[test]
    fn load_from_disk_rejects_expired_entry() {
        let _g = env_lock();
        let dir = tempdir().unwrap();
        with_cache_dir(dir.path(), || {
            let repo = std::path::Path::new("/tmp/repo-expired-xyz");
            let path = cache_path_for(repo).unwrap();
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let file = CacheFile {
                fetched_at: 0, // ancient
                repo: repo.to_string_lossy().into_owned(),
                prs: HashMap::new(),
            };
            std::fs::write(&path, serde_json::to_string(&file).unwrap()).unwrap();

            assert!(load_from_disk(repo).is_none());
        });
    }

    #[test]
    fn load_from_disk_rejects_corrupt_file() {
        let _g = env_lock();
        let dir = tempdir().unwrap();
        with_cache_dir(dir.path(), || {
            let repo = std::path::Path::new("/tmp/repo-corrupt-xyz");
            let path = cache_path_for(repo).unwrap();
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, "not json").unwrap();

            assert!(load_from_disk(repo).is_none());
        });
    }

    #[test]
    fn load_or_fetch_uses_disk_when_fresh() {
        let _g = env_lock();
        let dir = tempdir().unwrap();
        with_cache_dir(dir.path(), || {
            let repo = std::path::Path::new("/tmp/repo-disk-hit-xyz");
            let path = cache_path_for(repo).unwrap();
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let file = CacheFile {
                fetched_at: now_secs(),
                repo: repo.to_string_lossy().into_owned(),
                prs: [("feat/cached".to_string(), "MERGED".to_string())].into_iter().collect(),
            };
            std::fs::write(&path, serde_json::to_string(&file).unwrap()).unwrap();

            // No GW_TEST_GH_JSON set. gh must not be consulted; if it were
            // called in CI without a repo, it would fail — instead we get
            // the disk value.
            std::env::set_var("GW_TEST_GH_FAIL", "1");
            let cache = PrCache::load_or_fetch(repo, false);
            std::env::remove_var("GW_TEST_GH_FAIL");
            assert_eq!(cache.state("feat/cached"), Some("MERGED"));
        });
    }

    #[test]
    fn load_or_fetch_bypasses_disk_when_no_cache_true() {
        let _g = env_lock();
        let dir = tempdir().unwrap();
        with_cache_dir(dir.path(), || {
            let repo = std::path::Path::new("/tmp/repo-bypass-xyz");
            let path = cache_path_for(repo).unwrap();
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let file = CacheFile {
                fetched_at: now_secs(),
                repo: repo.to_string_lossy().into_owned(),
                prs: [("feat/old".to_string(), "OPEN".to_string())].into_iter().collect(),
            };
            std::fs::write(&path, serde_json::to_string(&file).unwrap()).unwrap();

            std::env::set_var(
                "GW_TEST_GH_JSON",
                r#"[{"headRefName":"feat/new","state":"OPEN"}]"#,
            );
            let cache = PrCache::load_or_fetch(repo, true);
            std::env::remove_var("GW_TEST_GH_JSON");
            assert_eq!(cache.state("feat/new"), Some("OPEN"));
            assert_eq!(cache.state("feat/old"), None);
        });
    }

    #[test]
    fn load_or_fetch_empty_when_gh_fails_and_no_cache_file() {
        let _g = env_lock();
        let dir = tempdir().unwrap();
        with_cache_dir(dir.path(), || {
            let repo = std::path::Path::new("/tmp/repo-empty-xyz");
            std::env::set_var("GW_TEST_GH_FAIL", "1");
            let cache = PrCache::load_or_fetch(repo, false);
            std::env::remove_var("GW_TEST_GH_FAIL");
            assert!(cache.state("anything").is_none());
        });
    }
}
