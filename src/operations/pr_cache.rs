//! Batched PR-status cache for `gw list`.
//!
//! Calls `gh pr list` once per `gw` invocation (instead of `gh pr view` per
//! worktree) and persists the result under
//! `~/.cache/gw/pr-status-<repo-hash>.json` with a 60-second TTL. On any
//! failure (gh missing, disk error, corrupt file), `PrCache::load_or_fetch`
//! returns an empty cache so callers fall back to `git branch --merged`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(not(test))]
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

/// Per-process counter appended to tmp filenames for nano-collision safety
/// when two threads or processes write the same repo cache within the same nanosecond.
static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Ensures the orphan-sweep runs at most once per process. Running it on every
/// write would add a `read_dir` syscall to every cache update; once per process
/// is sufficient because tmp files from prior runs are already old enough to sweep
/// and new ones created within this run will be renamed away or cleaned up inline.
#[cfg(not(test))]
static SWEEP_DONE: OnceLock<()> = OnceLock::new();

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// 60-second TTL — balances freshness against gh rate limits.
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
#[non_exhaustive]
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
    ///
    /// On failure, the on-disk cache (if any) is left untouched — only a
    /// successful fetch triggers a write.
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
    ///
    /// When `no_cache=true` and `gh` is down, the previous on-disk cache is
    /// preserved but not consulted; the next non-bypass call may serve stale
    /// data until `gh` recovers.
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
///
/// 16 hex chars / 64 bits — collision-free in practice for per-user repo counts.
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
        // #37: GW_TEST_GH_FAIL takes precedence over GW_TEST_GH_JSON — a test
        // that sets both will always get None, never the JSON payload.
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

/// Returns the current Unix timestamp in seconds, or `None` if the system
/// clock is broken (pre-epoch or overflow). `None` means: skip caching
/// entirely — broken clock = no TTL we can trust.
fn now_secs() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
}

/// Read cache file if it exists and is still within TTL. Any error → None.
/// Returns None if `now_secs()` is None (broken clock — safer to refetch).
fn load_from_disk(repo: &Path) -> Option<HashMap<String, PrState>> {
    let path = cache_path_for(repo)?;
    let data = std::fs::read_to_string(&path).ok()?;
    let file: CacheFile = serde_json::from_str(&data).ok()?;
    // broken clock (None) → refuse to serve possibly-stale cache
    let now = now_secs()?;
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

/// Remove orphaned `.tmp.` files from `parent` that are older than `cutoff`.
///
/// Orphans accumulate when a prior `gw` process crashed between the `fs::write`
/// and `fs::rename` steps. Best-effort: any I/O error is silently ignored.
fn sweep_orphans(parent: &Path, cutoff: SystemTime) {
    let Ok(entries) = std::fs::read_dir(parent) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if !(name_str.starts_with("pr-status-") && name_str.contains(".tmp.")) {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        let Ok(modified) = meta.modified() else {
            continue;
        };
        if modified < cutoff {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// Best-effort write. Failures are silently ignored — the in-memory result is
/// still returned to the caller.
///
/// On failure, the on-disk cache (if any) is left untouched.
///
/// #14/#23: `prs` is borrowed (&HashMap) and cloned into `CacheFile`. Taking
/// ownership would require `fetch_and_persist` to pass the map by value,
/// complicating the return-value path for no meaningful perf gain at typical
/// worktree counts (~10s of PRs). Kept as a borrow for clarity.
fn write_to_disk(repo: &Path, prs: &HashMap<String, PrState>) {
    let Some(path) = cache_path_for(repo) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // #10: compute secs and nanos from a single SystemTime::now() call so
    // both share the same instant — avoids a second clock call for nanos.
    // #13: broken clock means we cannot set a meaningful fetched_at timestamp
    // — skip persistence entirely; in-memory result is still returned to the
    // caller via fetch_and_persist.
    let now_instant = SystemTime::now();
    let dur = match now_instant.duration_since(UNIX_EPOCH) {
        Ok(d) => d,
        Err(_) => return, // broken clock
    };
    let now = dur.as_secs();
    let nanos = dur.subsec_nanos();

    let file = CacheFile {
        fetched_at: now,
        repo: repo.to_string_lossy().into_owned(),
        prs: prs.clone(),
    };
    let Ok(json) = serde_json::to_string(&file) else {
        return;
    };

    // Sweep orphans from prior failed runs (older than 60s) so the cache dir
    // doesn't accumulate cruft. Best-effort: any error is silently ignored.
    // Gated on SWEEP_DONE so the read_dir syscall runs at most once per process —
    // orphans from prior runs are already old; ones from this run are handled inline.
    // In test builds the gate is skipped so each test gets a deterministic sweep.
    #[cfg(not(test))]
    let do_sweep = SWEEP_DONE.set(()).is_ok();
    #[cfg(test)]
    let do_sweep = true;
    if do_sweep {
        if let Some(parent) = path.parent() {
            // On systems with a clock < 60s past epoch, this collapses to "no orphans
            // older than `now`", effectively skipping the sweep — acceptable degenerate.
            let cutoff = SystemTime::now()
                .checked_sub(std::time::Duration::from_secs(60))
                .unwrap_or_else(SystemTime::now);
            sweep_orphans(parent, cutoff);
        }
    }

    // Atomic write: write to <path>.tmp.<pid>.<nanos>.<counter>, then rename.
    // Using pid + nanoseconds + per-process counter avoids collisions when:
    //   - multiple gw processes write concurrently (different pid), or
    //   - two writes happen within the same nanosecond (counter breaks the tie).
    // On Windows, std::fs::rename fails if the target exists; we retry with a
    // remove-then-rename fallback (best-effort, second failure is silently ignored).
    // #11/#8: use file_stem to strip .json before the tmp suffix, giving
    // "pr-status-<hash>.tmp.PID.NANOS.COUNTER" — shorter and groups cleanly
    // with the final file. Using file_name would produce "pr-status-<hash>.json.tmp…".
    let counter = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp = path.with_file_name(format!(
        "{}.tmp.{}.{}.{}",
        path.file_stem().unwrap_or_default().to_string_lossy(),
        std::process::id(),
        nanos,
        counter,
    ));
    // #15/#34: clean up the tmp file on initial write failure. `remove_file`
    // is best-effort — if write never created the file (e.g. permission error
    // before any bytes were written) the ENOENT is silently ignored via `.ok()`.
    if std::fs::write(&tmp, &json).is_err() {
        let _ = std::fs::remove_file(&tmp); // ENOENT is harmless here
        return;
    }
    if std::fs::rename(&tmp, &path).is_err() {
        // #12: Windows fallback: target may already exist; best-effort remove then
        // retry. If the second rename also fails, remove the orphaned tmp file.
        // Best-effort: another process can race the rename and leave an orphan tmp
        // file, but those will be reaped on the next successful write.
        let _ = std::fs::remove_file(&path);
        if std::fs::rename(&tmp, &path).is_err() {
            let _ = std::fs::remove_file(&tmp); // cleanup orphan
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // The env-var lock and guard are defined in `super::super::test_env` so
    // this module and `clean` share a single mutex. Without sharing, each
    // module's tests would hold a different lock and race on the same global
    // env vars — which surfaced as a flaky CI failure of
    // `fetch_parses_gh_json_from_env` and
    // `load_or_fetch_bypasses_disk_when_no_cache_true` once `clean::tests`
    // started using the same env vars.
    use super::super::test_env::{env_lock, EnvGuard};

    /// Sanity-check that now_secs() works on a normal system.
    // Note: the `None` branch (broken clock) is not exercised by tests; it requires
    // time manipulation. Coverage gap accepted.
    #[test]
    fn now_secs_returns_some_on_normal_system() {
        assert!(now_secs().is_some());
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
        let _env = EnvGuard::capture(&["GW_TEST_GH_FAIL", "GW_TEST_GH_JSON"]);
        std::env::set_var(
            "GW_TEST_GH_JSON",
            r#"[{"headRefName":"feat/foo","state":"OPEN"},{"headRefName":"fix/bar","state":"MERGED"}]"#,
        );
        let prs = fetch_from_gh(std::path::Path::new(".")).expect("parsed");
        assert_eq!(prs.get("feat/foo"), Some(&PrState::Open));
        assert_eq!(prs.get("fix/bar"), Some(&PrState::Merged));
    }

    #[test]
    fn fetch_returns_none_on_forced_failure() {
        let _g = env_lock();
        let _env = EnvGuard::capture(&["GW_TEST_GH_FAIL", "GW_TEST_GH_JSON"]);
        std::env::set_var("GW_TEST_GH_FAIL", "1");
        let result = fetch_from_gh(std::path::Path::new("."));
        assert!(result.is_none());
    }

    use tempfile::tempdir;

    /// Set `GW_TEST_CACHE_DIR` for the duration of `f`. Restores the previous
    /// value (or removes the var) via an `EnvGuard`, so the env is cleaned up
    /// even if `f` panics.
    fn with_cache_dir<F: FnOnce()>(dir: &std::path::Path, f: F) {
        let _g = EnvGuard::capture(&["GW_TEST_CACHE_DIR"]);
        std::env::set_var("GW_TEST_CACHE_DIR", dir);
        f();
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
            let far_future = now_secs().unwrap() + 9999;
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
        let _env = EnvGuard::capture(&["GW_TEST_CACHE_DIR", "GW_TEST_GH_FAIL", "GW_TEST_GH_JSON"]);
        let dir = tempdir().unwrap();
        with_cache_dir(dir.path(), || {
            let repo = std::path::Path::new("/tmp/repo-disk-hit-xyz");
            let path = cache_path_for(repo).unwrap();
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let file = CacheFile {
                fetched_at: now_secs().unwrap(),
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
            assert_eq!(cache.state("feat/cached"), Some(&PrState::Merged));
        });
    }

    #[test]
    fn load_or_fetch_bypasses_disk_when_no_cache_true() {
        let _g = env_lock();
        let _env = EnvGuard::capture(&["GW_TEST_CACHE_DIR", "GW_TEST_GH_FAIL", "GW_TEST_GH_JSON"]);
        let dir = tempdir().unwrap();
        with_cache_dir(dir.path(), || {
            let repo = std::path::Path::new("/tmp/repo-bypass-xyz");
            let path = cache_path_for(repo).unwrap();
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let file = CacheFile {
                fetched_at: now_secs().unwrap(),
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
            assert_eq!(cache.state("feat/new"), Some(&PrState::Open));
            assert_eq!(cache.state("feat/old"), None);
        });
    }

    #[test]
    fn load_or_fetch_empty_when_gh_fails_and_no_cache_file() {
        let _g = env_lock();
        let _env = EnvGuard::capture(&["GW_TEST_CACHE_DIR", "GW_TEST_GH_FAIL", "GW_TEST_GH_JSON"]);
        let dir = tempdir().unwrap();
        with_cache_dir(dir.path(), || {
            let repo = std::path::Path::new("/tmp/repo-empty-xyz");
            std::env::set_var("GW_TEST_GH_FAIL", "1");
            let cache = PrCache::load_or_fetch(repo, false);
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
        let _env = EnvGuard::capture(&["GW_TEST_CACHE_DIR", "GW_TEST_GH_FAIL", "GW_TEST_GH_JSON"]);
        let dir = tempdir().unwrap();
        with_cache_dir(dir.path(), || {
            let repo = std::path::Path::new("/tmp/repo-split-xyz");
            // from_disk returns None when no file exists
            assert!(PrCache::from_disk(repo).is_none());

            // fetch_and_persist falls back to empty on gh failure
            std::env::set_var("GW_TEST_GH_FAIL", "1");
            let empty = PrCache::fetch_and_persist(repo);
            assert!(empty.state("anything").is_none());
            // Transition to success phase: clear FAIL so GH_JSON is consulted.
            std::env::remove_var("GW_TEST_GH_FAIL");

            // fetch_and_persist writes to disk on success
            std::env::set_var(
                "GW_TEST_GH_JSON",
                r#"[{"headRefName":"main","state":"OPEN"}]"#,
            );
            let _ = PrCache::fetch_and_persist(repo);
            // from_disk now returns the written file
            let loaded = PrCache::from_disk(repo).expect("written to disk");
            assert_eq!(loaded.state("main"), Some(&PrState::Open));
        });
    }

    /// Verify that write_to_disk removes orphaned .tmp.* files older than 60s.
    #[cfg(unix)]
    #[test]
    fn write_to_disk_sweeps_old_orphan_tmp_files() {
        let _g = env_lock();
        let _env = EnvGuard::capture(&["GW_TEST_CACHE_DIR"]);
        let dir = tempdir().unwrap();
        with_cache_dir(dir.path(), || {
            let repo = std::path::Path::new("/tmp/repo-sweep-xyz");
            let final_path = cache_path_for(repo).unwrap();
            let parent = final_path.parent().unwrap();
            std::fs::create_dir_all(parent).unwrap();

            // Plant an old orphan tmp file and backdate its mtime to epoch
            // (clearly older than the 60s sweep cutoff).
            let orphan = parent.join("pr-status-orphan.tmp.99999.123456789.0");
            std::fs::write(&orphan, "stale").unwrap();
            // Set mtime to Unix epoch via libc::utimes (available on unix).
            {
                use std::ffi::CString;
                let c_path = CString::new(orphan.to_string_lossy().as_bytes()).unwrap();
                let times = [libc::timeval {
                    tv_sec: 0,
                    tv_usec: 0,
                }; 2];
                // SAFETY: c_path is valid, times array is correctly sized.
                unsafe { libc::utimes(c_path.as_ptr(), times.as_ptr()) };
            }

            // Plant a fresh tmp file with current mtime — the sweep must NOT remove it.
            let fresh_tmp = parent.join("pr-status-fresh.tmp.123.456.0");
            std::fs::write(&fresh_tmp, "fresh").unwrap();

            // Trigger a write — the sweep runs before writing the tmp file.
            let mut prs = HashMap::new();
            prs.insert("feat/sweep".to_string(), PrState::Open);
            write_to_disk(repo, &prs);

            assert!(
                !orphan.exists(),
                "old orphan tmp file should have been swept"
            );
            assert!(fresh_tmp.exists(), "fresh tmp file should not be swept");
            assert!(final_path.exists(), "final cache file should exist");
        });
    }

    /// Canary: ensures every PrState variant is enumerated. A new variant
    /// breaks compilation here, forcing the author to inspect callers.
    #[test]
    fn pr_state_variants_are_handled() {
        // The value is the exhaustiveness check; runtime asserts are noise.
        fn _must_handle(s: &PrState) {
            match s {
                PrState::Open => {}
                PrState::Merged => {}
                PrState::Closed => {}
                PrState::Other => {}
            }
        }
    }
}
