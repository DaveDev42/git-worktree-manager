//! Batched PR-status cache for `gw list`.
//!
//! Calls `gh pr list` once per `gw` invocation (instead of `gh pr view` per
//! worktree) and persists the result under
//! `~/.cache/gw/pr-status-<repo-hash>.json` with a 60-second TTL. On any
//! failure (gh missing, disk error, corrupt file), `PrCache::load_or_fetch`
//! returns an empty cache so callers fall back to `git branch --merged`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// TODO(config): make TTL configurable via GW_PR_CACHE_TTL env var in a
// follow-up (config surface decision deferred from review).
const CACHE_TTL_SECS: u64 = 60;

/// Cap on PRs fetched per `gh pr list` call. Repos with more PRs will see the
/// oldest fall back to git-only merge detection.
///
/// If `prs.len() == GH_FETCH_LIMIT` we may be missing older entries; consider
/// paginating in a follow-up.
const GH_FETCH_LIMIT: usize = 500;

/// Typed PR state as returned by `gh pr list`.
///
/// The `#[serde(other)]` variant catches any future states GitHub may add
/// without breaking deserialization.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum PrState {
    Open,
    Merged,
    Closed,
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheFile {
    fetched_at: u64,
    repo: String,
    prs: HashMap<String, PrState>,
}

#[derive(Debug, Default, Clone)]
pub struct PrCache {
    map: HashMap<String, PrState>,
}

impl PrCache {
    /// Return the PR state for `branch`, if known.
    ///
    /// `branch` must be in the same form that `gh pr list` returns for
    /// `headRefName` — i.e. **without** a `refs/heads/` prefix. Callers in
    /// `display.rs` pass `branch_name` which comes from
    /// `git::normalize_branch_name`, which strips `refs/heads/` so the form
    /// matches `gh`'s output.
    pub fn state(&self, branch: &str) -> Option<&PrState> {
        self.map.get(branch)
    }

    /// Try loading a fresh cache entry from disk. Returns `None` if the file
    /// is missing, expired, corrupt, or in the future (clock skew guard).
    pub fn from_disk(repo: &Path) -> Option<Self> {
        load_from_disk(repo).map(|map| PrCache { map })
    }

    /// Fetch PR state via `gh pr list` and persist to disk. Returns an empty
    /// cache on any failure so callers' fallback path still works.
    pub fn fetch_and_persist(repo: &Path) -> Self {
        match fetch_from_gh(repo) {
            Some(map) => {
                write_to_disk(repo, &map);
                PrCache { map }
            }
            None => PrCache::default(),
        }
    }

    /// Load from disk if fresh (and `no_cache` is false), else fetch via
    /// `gh pr list` and persist. Returns an empty cache on any failure so
    /// the caller's fallback path still works.
    pub fn load_or_fetch(repo: &Path, no_cache: bool) -> Self {
        if !no_cache {
            if let Some(c) = Self::from_disk(repo) {
                return c;
            }
        }
        Self::fetch_and_persist(repo)
    }
}

/// Compute a stable short hash for a repository path.
/// Canonicalizes so `/foo/../foo` hashes the same as `/foo`.
///
/// If canonicalization fails (transient FS issue), fall back to the raw path.
/// Caches keyed on raw vs canonical paths will be different but self-consistent.
fn repo_hash(repo: &Path) -> String {
    let canon = repo.canonicalize().unwrap_or_else(|_| repo.to_path_buf());
    let mut hasher = Sha256::new();
    hasher.update(canon.to_string_lossy().as_bytes());
    let digest = hasher.finalize();
    hex_short(&digest[..8])
}

fn hex_short(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(out, "{:02x}", b);
    }
    out
}

/// Return the on-disk cache path for a given repo.
/// Returns None if we cannot determine a cache directory on this platform.
fn cache_path_for(repo: &Path) -> Option<PathBuf> {
    #[cfg(test)]
    if let Ok(dir) = std::env::var("GW_TEST_CACHE_DIR") {
        return Some(
            PathBuf::from(dir)
                .join("gw")
                .join(format!("pr-status-{}.json", repo_hash(repo))),
        );
    }

    let base = dirs::cache_dir()?.join("gw");
    Some(base.join(format!("pr-status-{}.json", repo_hash(repo))))
}

#[derive(Debug, Deserialize)]
struct GhPr {
    #[serde(rename = "headRefName")]
    head_ref_name: String,
    state: PrState,
}

/// Run `gh pr list --state all --json headRefName,state --limit N` and parse.
/// Returns None on any failure (gh missing, non-zero exit, JSON parse error).
///
/// Parse failure swallows the error per spec's silent-fallback contract.
fn fetch_from_gh(repo: &Path) -> Option<HashMap<String, PrState>> {
    #[cfg(test)]
    {
        if std::env::var("GW_TEST_GH_FAIL").ok().as_deref() == Some("1") {
            return None;
        }
        if let Ok(json) = std::env::var("GW_TEST_GH_JSON") {
            let prs: Vec<GhPr> = serde_json::from_str(json.trim()).ok()?;
            let mut map = HashMap::with_capacity(prs.len());
            for pr in prs {
                map.insert(pr.head_ref_name, pr.state);
            }
            return Some(map);
        }
    }

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

    let prs: Vec<GhPr> = serde_json::from_str(result.stdout.trim()).ok()?;
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
fn load_from_disk(repo: &Path) -> Option<HashMap<String, PrState>> {
    let path = cache_path_for(repo)?;
    let data = std::fs::read_to_string(&path).ok()?;
    let file: CacheFile = serde_json::from_str(&data).ok()?;
    let now = now_secs();
    // Reject entries from the future (clock skew guard).
    if file.fetched_at > now {
        return None;
    }
    let age = now.saturating_sub(file.fetched_at);
    if age > CACHE_TTL_SECS {
        return None;
    }
    Some(file.prs)
}

/// Best-effort write. Failures are silently ignored — the in-memory result is
/// still returned to the caller.
///
/// TODO(perf): avoid prs.clone() by taking ownership; deferred as premature
/// optimization for this PR.
fn write_to_disk(repo: &Path, prs: &HashMap<String, PrState>) {
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

    // Atomic write: write to <path>.tmp.<pid>.<nanos>, then rename.
    // Using both pid and nanoseconds avoids collisions when multiple gw
    // processes write concurrently (different pid) or rapidly (different nanos).
    // On Windows, std::fs::rename fails if the target exists; we retry with a
    // remove-then-rename fallback (best-effort, second failure is silently ignored).
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let tmp = path.with_extension(format!("tmp.{}.{}", std::process::id(), nanos));
    if std::fs::write(&tmp, json).is_ok() && std::fs::rename(&tmp, &path).is_err() {
        // Windows fallback: target may already exist; best-effort remove then retry.
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::rename(&tmp, &path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::{Mutex, MutexGuard};

    // Tests mutate process-global env vars; the mutex serializes them to avoid
    // races. Production code does not consult these vars (see #[cfg(test)]
    // gates above).
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
        assert_eq!(prs.get("feat/foo"), Some(&PrState::Open));
        assert_eq!(prs.get("fix/bar"), Some(&PrState::Merged));
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
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let file = CacheFile {
                fetched_at: now,
                repo: repo.to_string_lossy().into_owned(),
                prs: [("feat/a".to_string(), PrState::Open)]
                    .into_iter()
                    .collect(),
            };
            std::fs::write(&path, serde_json::to_string(&file).unwrap()).unwrap();

            let loaded = load_from_disk(repo).expect("fresh cache");
            assert_eq!(loaded.get("feat/a"), Some(&PrState::Open));
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
    fn load_from_disk_rejects_future_entry() {
        let _g = env_lock();
        let dir = tempdir().unwrap();
        with_cache_dir(dir.path(), || {
            let repo = std::path::Path::new("/tmp/repo-future-xyz");
            let path = cache_path_for(repo).unwrap();
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let far_future = now_secs() + 9999;
            let file = CacheFile {
                fetched_at: far_future,
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
                prs: [("feat/cached".to_string(), PrState::Merged)]
                    .into_iter()
                    .collect(),
            };
            std::fs::write(&path, serde_json::to_string(&file).unwrap()).unwrap();

            // No GW_TEST_GH_JSON set. gh must not be consulted; if it were
            // called in CI without a repo, it would fail — instead we get
            // the disk value.
            std::env::set_var("GW_TEST_GH_FAIL", "1");
            let cache = PrCache::load_or_fetch(repo, false);
            std::env::remove_var("GW_TEST_GH_FAIL");
            assert_eq!(cache.state("feat/cached"), Some(&PrState::Merged));
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
                prs: [("feat/old".to_string(), PrState::Open)]
                    .into_iter()
                    .collect(),
            };
            std::fs::write(&path, serde_json::to_string(&file).unwrap()).unwrap();

            std::env::set_var(
                "GW_TEST_GH_JSON",
                r#"[{"headRefName":"feat/new","state":"OPEN"}]"#,
            );
            let cache = PrCache::load_or_fetch(repo, true);
            std::env::remove_var("GW_TEST_GH_JSON");
            assert_eq!(cache.state("feat/new"), Some(&PrState::Open));
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

    #[test]
    fn write_to_disk_cleans_up_tmp_file() {
        let _g = env_lock();
        let dir = tempdir().unwrap();
        with_cache_dir(dir.path(), || {
            let repo = std::path::Path::new("/tmp/repo-atomic-xyz");
            let mut prs = HashMap::new();
            prs.insert("feat/x".to_string(), PrState::Open);
            write_to_disk(repo, &prs);

            let final_path = cache_path_for(repo).unwrap();
            assert!(final_path.exists(), "final cache file exists");

            // The .tmp.<pid>.<nanos> file should have been renamed away.
            let parent = final_path.parent().unwrap();
            let entries: Vec<_> = std::fs::read_dir(parent).unwrap().flatten().collect();
            for entry in &entries {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                assert!(
                    !name_str.contains(".tmp."),
                    "no tmp file should remain: {}",
                    name_str
                );
            }
        });
    }

    #[test]
    fn from_disk_and_fetch_and_persist_split() {
        let _g = env_lock();
        let dir = tempdir().unwrap();
        with_cache_dir(dir.path(), || {
            let repo = std::path::Path::new("/tmp/repo-split-xyz");
            // from_disk returns None when no file exists
            assert!(PrCache::from_disk(repo).is_none());

            // fetch_and_persist falls back to empty on gh failure
            std::env::set_var("GW_TEST_GH_FAIL", "1");
            let empty = PrCache::fetch_and_persist(repo);
            std::env::remove_var("GW_TEST_GH_FAIL");
            assert!(empty.state("anything").is_none());

            // fetch_and_persist writes to disk on success
            std::env::set_var(
                "GW_TEST_GH_JSON",
                r#"[{"headRefName":"main","state":"OPEN"}]"#,
            );
            let _ = PrCache::fetch_and_persist(repo);
            std::env::remove_var("GW_TEST_GH_JSON");
            // from_disk now returns the written file
            let loaded = PrCache::from_disk(repo).expect("written to disk");
            assert_eq!(loaded.state("main"), Some(&PrState::Open));
        });
    }
}
