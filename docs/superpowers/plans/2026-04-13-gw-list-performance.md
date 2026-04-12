# gw list Performance & Progressive Rendering Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `gw list` respond instantaneously in repos with many worktrees by batching `gh` calls, caching PR state, parallelizing per-worktree work with rayon, and rendering the table progressively using ratatui Inline Viewport.

**Architecture:** Add a new `PrCache` module that wraps a single `gh pr list` call with an on-disk cache at `~/.cache/gw/pr-status-<repo-hash>.json` (60s TTL). Change `get_worktree_status` to accept `&PrCache`. Parallelize per-worktree status computation with rayon. In TTY mode, render via a new `src/tui/` layer built on ratatui's `Viewport::Inline`, showing a skeleton table immediately and filling statuses as they complete. Non-TTY falls back to static output.

**Tech Stack:** Rust, clap, ratatui 0.28, crossterm 0.28, rayon 1.10, sha2 0.10, serde_json, dirs.

**Spec:** `docs/superpowers/specs/2026-04-13-gw-list-performance-design.md`

---

## File Structure

**New files:**
- `src/operations/pr_cache.rs` — batched `gh pr list` + XDG cache with TTL
- `src/tui/mod.rs` — module root, panic-hook installer, TTY detection helper
- `src/tui/style.rs` — status → `ratatui::Style` mapping (mirrors `console` palette)
- `src/tui/list_view.rs` — Inline Viewport app for `gw list`

**Modified files:**
- `Cargo.toml` — add `ratatui`, `crossterm`, `rayon`, `sha2`
- `src/lib.rs` — declare `tui` module
- `src/main.rs` — pass `no_cache` flag, install panic hook, route List to new code path
- `src/cli.rs` — add `--no-cache` to `List`
- `src/operations/display.rs` — `get_worktree_status` accepts `&PrCache`; `list_worktrees` dispatches TTY vs static
- `src/operations/global_ops.rs`, `src/operations/diagnostics.rs`, `src/operations/clean.rs` — update callers to build/pass `PrCache`
- `src/operations/mod.rs` — export `pr_cache`

---

## Task 1: Add Dependencies

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Edit `[dependencies]` block**

Add these four lines to the `[dependencies]` section of `Cargo.toml` (alongside existing entries like `dirs = "6"`):

```toml
ratatui = "0.28"
crossterm = "0.28"
rayon = "1.10"
sha2 = "0.10"
```

- [ ] **Step 2: Verify build**

Run: `cargo build`
Expected: builds successfully; new crates appear in `Cargo.lock`. Zero compiler errors.

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore(deps): add ratatui, crossterm, rayon, sha2 for gw list perf"
```

---

## Task 2: PrCache Module — Skeleton & Repo Hash

**Files:**
- Create: `src/operations/pr_cache.rs`
- Modify: `src/operations/mod.rs`

- [ ] **Step 1: Create `src/operations/pr_cache.rs` with struct + repo hash helper**

```rust
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
```

- [ ] **Step 2: Register module in `src/operations/mod.rs`**

Open `src/operations/mod.rs` and add `pub mod pr_cache;` alongside the other `pub mod` declarations (alphabetical order preferred).

- [ ] **Step 3: Run tests**

Run: `cargo test -p git-worktree-manager pr_cache -- --nocapture`
Expected: 3 tests pass (`repo_hash_is_stable_and_short`, `repo_hash_differs_per_path`, `cache_path_contains_repo_hash`).

- [ ] **Step 4: Commit**

```bash
git add src/operations/pr_cache.rs src/operations/mod.rs
git commit -m "feat(pr_cache): add module skeleton with repo-hash and cache path"
```

---

## Task 3: PrCache Module — gh Fetch with Test Hook

**Files:**
- Modify: `src/operations/pr_cache.rs`

- [ ] **Step 1: Add failing test for the fetch path**

Append to the `tests` module inside `src/operations/pr_cache.rs`:

```rust
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
```

- [ ] **Step 2: Run and confirm it fails**

Run: `cargo test -p git-worktree-manager pr_cache::tests::fetch -- --nocapture`
Expected: FAIL — `fetch_from_gh` does not exist.

- [ ] **Step 3: Implement `fetch_from_gh`**

Add to `src/operations/pr_cache.rs` (above the `tests` module):

```rust
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
```

- [ ] **Step 4: Check that `crate::git::has_command` and `crate::git::run_command` are accessible**

Run: `grep -n "pub fn has_command\|pub fn run_command" /Users/dave/Projects/github.com/git-worktree-manager/src/git.rs`
Expected output contains both `pub fn has_command` and `pub fn run_command`. If either is private, mark it `pub(crate)` in `src/git.rs` as a one-line change in this same commit.

- [ ] **Step 5: Run tests**

Run: `cargo test -p git-worktree-manager pr_cache -- --nocapture`
Expected: all 5 tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/operations/pr_cache.rs src/git.rs
git commit -m "feat(pr_cache): fetch PR state from gh in one batched call"
```

---

## Task 4: PrCache Module — Disk Read/Write with TTL

**Files:**
- Modify: `src/operations/pr_cache.rs`

- [ ] **Step 1: Add failing tests for TTL and corrupt-file behavior**

Append to the `tests` module:

```rust
    use tempfile::tempdir;

    fn with_cache_dir<F: FnOnce()>(dir: &std::path::Path, f: F) {
        let prev = std::env::var_os("XDG_CACHE_HOME");
        std::env::set_var("XDG_CACHE_HOME", dir);
        f();
        match prev {
            Some(v) => std::env::set_var("XDG_CACHE_HOME", v),
            None => std::env::remove_var("XDG_CACHE_HOME"),
        }
    }

    #[test]
    fn load_from_disk_returns_fresh_entry() {
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
        let dir = tempdir().unwrap();
        with_cache_dir(dir.path(), || {
            let repo = std::path::Path::new("/tmp/repo-corrupt-xyz");
            let path = cache_path_for(repo).unwrap();
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, "not json").unwrap();

            assert!(load_from_disk(repo).is_none());
        });
    }
```

- [ ] **Step 2: Run and confirm failures**

Run: `cargo test -p git-worktree-manager pr_cache::tests::load_from_disk -- --nocapture`
Expected: FAIL — `load_from_disk` / `write_to_disk` not defined.

- [ ] **Step 3: Implement disk helpers**

Add to `src/operations/pr_cache.rs` (above `tests`):

```rust
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
    if let Ok(json) = serde_json::to_string(&file) {
        let _ = std::fs::write(&path, json);
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p git-worktree-manager pr_cache -- --nocapture`
Expected: 8 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/operations/pr_cache.rs
git commit -m "feat(pr_cache): persist PR state to disk with 60s TTL"
```

---

## Task 5: PrCache Module — Public `load_or_fetch`

**Files:**
- Modify: `src/operations/pr_cache.rs`

- [ ] **Step 1: Add failing tests for orchestration**

Append to the `tests` module:

```rust
    #[test]
    fn load_or_fetch_uses_disk_when_fresh() {
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
        let dir = tempdir().unwrap();
        with_cache_dir(dir.path(), || {
            let repo = std::path::Path::new("/tmp/repo-empty-xyz");
            std::env::set_var("GW_TEST_GH_FAIL", "1");
            let cache = PrCache::load_or_fetch(repo, false);
            std::env::remove_var("GW_TEST_GH_FAIL");
            assert!(cache.state("anything").is_none());
        });
    }
```

- [ ] **Step 2: Run and confirm failures**

Run: `cargo test -p git-worktree-manager pr_cache::tests::load_or_fetch -- --nocapture`
Expected: FAIL — `PrCache::load_or_fetch` not implemented.

- [ ] **Step 3: Implement the public API**

Add to `impl PrCache` in `src/operations/pr_cache.rs`:

```rust
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
```

- [ ] **Step 4: Run full pr_cache suite**

Run: `cargo test -p git-worktree-manager pr_cache -- --nocapture`
Expected: 11 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/operations/pr_cache.rs
git commit -m "feat(pr_cache): public load_or_fetch with --no-cache semantics"
```

---

## Task 6: Thread `PrCache` Through `get_worktree_status`

**Files:**
- Modify: `src/operations/display.rs`
- Modify: `src/operations/global_ops.rs`
- Modify: `src/operations/diagnostics.rs`
- Modify: `src/operations/clean.rs`

- [ ] **Step 1: Update `get_worktree_status` signature and body**

In `src/operations/display.rs`, replace the current `pub fn get_worktree_status(...)` signature and the `get_pr_state` call site. The new version:

```rust
pub fn get_worktree_status(
    path: &Path,
    repo: &Path,
    branch: Option<&str>,
    pr_cache: &crate::operations::pr_cache::PrCache,
) -> String {
    if !path.exists() {
        return "stale".to_string();
    }

    if !crate::operations::busy::detect_busy(path).is_empty() {
        return "busy".to_string();
    }

    if let Ok(cwd) = std::env::current_dir() {
        let cwd_canon = cwd.canonicalize().unwrap_or(cwd);
        let path_canon = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        if cwd_canon.starts_with(&path_canon) {
            return "active".to_string();
        }
    }

    if let Some(branch_name) = branch {
        let base_branch = {
            let key = format_config_key(CONFIG_KEY_BASE_BRANCH, branch_name);
            git::get_config(&key, Some(repo))
                .unwrap_or_else(|| git::detect_default_branch(Some(repo)))
        };

        // Primary: cached PR state from a single `gh pr list` call.
        if let Some(state) = pr_cache.state(branch_name) {
            match state {
                "MERGED" => return "merged".to_string(),
                "OPEN" => return "pr-open".to_string(),
                _ => {}
            }
        }

        // Fallback: git branch --merged (merge-commit strategy only).
        if git::is_branch_merged(branch_name, &base_branch, Some(repo)) {
            return "merged".to_string();
        }
    }

    if let Ok(result) = git::git_command(&["status", "--porcelain"], Some(path), false, true) {
        if result.returncode == 0 && !result.stdout.trim().is_empty() {
            return "modified".to_string();
        }
    }

    "clean".to_string()
}
```

- [ ] **Step 2: Update existing tests in `display.rs` that call `get_worktree_status`**

In `src/operations/display.rs`, the two test call sites (around lines 870 and 884) need an extra argument. Find each and add `&crate::operations::pr_cache::PrCache::default()`:

```rust
        let status = get_worktree_status(&wt, repo, Some("wt1"), &crate::operations::pr_cache::PrCache::default());
```

```rust
        assert_eq!(
            get_worktree_status(&non_existent, &repo, None, &crate::operations::pr_cache::PrCache::default()),
            "stale"
        );
```

Also update the two internal call sites at `src/operations/display.rs:398` and `:497` to accept a `pr_cache` parameter from the enclosing function — leave this for Task 7 when `list_worktrees` wiring changes. For now, temporarily pass `&crate::operations::pr_cache::PrCache::default()` at those two sites to keep the build green.

- [ ] **Step 3: Update `src/operations/global_ops.rs:96`**

Replace the call:

```rust
            let status = get_worktree_status(path, repo_path, Some(branch_name.as_str()));
```

with:

```rust
            let pr_cache = crate::operations::pr_cache::PrCache::load_or_fetch(repo_path, false);
            let status = get_worktree_status(path, repo_path, Some(branch_name.as_str()), &pr_cache);
```

(If this call is inside a loop, lift `pr_cache` construction above the loop so `gh` is only invoked once per repo.)

- [ ] **Step 4: Update `src/operations/diagnostics.rs:110`**

Find the enclosing function that iterates worktrees; construct a `PrCache` once before the loop and pass it into `get_worktree_status`:

```rust
            let status = get_worktree_status(path, repo, Some(branch_name.as_str()), &pr_cache);
```

Add `let pr_cache = crate::operations::pr_cache::PrCache::load_or_fetch(repo, false);` before the loop.

- [ ] **Step 5: Update `src/operations/clean.rs:85`**

Same pattern: lift `let pr_cache = crate::operations::pr_cache::PrCache::load_or_fetch(&repo, false);` above the loop, then:

```rust
            let status = get_worktree_status(&path, &repo, Some(branch_name.as_str()), &pr_cache);
```

- [ ] **Step 6: Build and run tests**

Run: `cargo build && cargo test`
Expected: builds with zero warnings. All existing tests still pass.

- [ ] **Step 7: Commit**

```bash
git add src/operations/display.rs src/operations/global_ops.rs src/operations/diagnostics.rs src/operations/clean.rs
git commit -m "refactor(display): thread PrCache through get_worktree_status"
```

---

## Task 7: Parallelize Static `list_worktrees`

**Files:**
- Modify: `src/operations/display.rs`

- [ ] **Step 1: Add rayon import at top of `display.rs`**

After the existing `use` block in `src/operations/display.rs`, add:

```rust
use rayon::prelude::*;
```

- [ ] **Step 2: Refactor `list_worktrees` to parallelize row computation**

Replace the current body of `pub fn list_worktrees() -> Result<()>` (lines ~123–198 per spec) with:

```rust
pub fn list_worktrees(no_cache: bool) -> Result<()> {
    let repo = git::get_repo_root(None)?;
    let worktrees = git::parse_worktrees(&repo)?;

    println!(
        "\n{}  {}\n",
        style("Worktrees for repository:").cyan().bold(),
        repo.display()
    );

    let pr_cache = crate::operations::pr_cache::PrCache::load_or_fetch(&repo, no_cache);

    // Serial prep: cheap local work, keep single-threaded for clarity.
    struct RowInput {
        path: std::path::PathBuf,
        current_branch: String,
        worktree_id: String,
        age: String,
        rel_path: String,
    }

    let inputs: Vec<RowInput> = worktrees
        .iter()
        .map(|(branch, path)| {
            let current_branch = git::normalize_branch_name(branch).to_string();
            let rel_path = pathdiff::diff_paths(path, &repo)
                .map(|p: std::path::PathBuf| p.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string_lossy().to_string());
            let age = path_age_str(path);
            let intended_branch = lookup_intended_branch(&repo, &current_branch, path);
            let worktree_id = intended_branch.unwrap_or_else(|| current_branch.clone());
            RowInput {
                path: path.clone(),
                current_branch,
                worktree_id,
                age,
                rel_path,
            }
        })
        .collect();

    // Parallel: I/O-bound per-worktree status work.
    let rows: Vec<WorktreeRow> = inputs
        .par_iter()
        .map(|i| {
            let status = get_worktree_status(&i.path, &repo, Some(&i.current_branch), &pr_cache);
            WorktreeRow {
                worktree_id: i.worktree_id.clone(),
                current_branch: i.current_branch.clone(),
                status,
                age: i.age.clone(),
                rel_path: i.rel_path.clone(),
            }
        })
        .collect();

    let term_width = cwconsole::terminal_width();
    if term_width >= MIN_TABLE_WIDTH {
        print_worktree_table(&rows);
    } else {
        print_worktree_compact(&rows);
    }

    let feature_count = if rows.len() > 1 { rows.len() - 1 } else { 0 };
    if feature_count > 0 {
        let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for row in &rows {
            *counts.entry(row.status.as_str()).or_insert(0) += 1;
        }

        let mut summary_parts = Vec::new();
        for &status_name in &[
            "clean", "modified", "busy", "active", "pr-open", "merged", "stale",
        ] {
            if let Some(&count) = counts.get(status_name) {
                if count > 0 {
                    let styled = cwconsole::status_style(status_name)
                        .apply_to(format!("{} {}", count, status_name));
                    summary_parts.push(styled.to_string());
                }
            }
        }

        let summary = if summary_parts.is_empty() {
            format!("\n{} feature worktree(s)", feature_count)
        } else {
            format!(
                "\n{} feature worktree(s) — {}",
                feature_count,
                summary_parts.join(", ")
            )
        };
        println!("{}", summary);
    }

    println!();
    Ok(())
}
```

- [ ] **Step 3: Update the two temporary `&PrCache::default()` call sites from Task 6**

The sites at lines ~398 and ~497 of `display.rs` belong to functions (likely `show_status` or similar) that also iterate worktrees. Lift a `PrCache::load_or_fetch` above each loop and pass it in. Concretely, the enclosing function gains a local:

```rust
    let pr_cache = crate::operations::pr_cache::PrCache::load_or_fetch(&repo, false);
```

and the inner call becomes:

```rust
        let status = get_worktree_status(path, &repo, Some(branch_name.as_str()), &pr_cache);
```

- [ ] **Step 4: Build and test**

Run: `cargo build && cargo test`
Expected: builds, all tests pass. `list_worktrees` signature change means callers must update in Task 8.

Note: this step may fail because `list_worktrees()` is now `list_worktrees(no_cache: bool)`. If so, temporarily call `list_worktrees(false)` from `src/main.rs` at the List dispatch in `src/main.rs:53` to unblock, then Task 8 adds the real plumbing.

- [ ] **Step 5: Commit**

```bash
git add src/operations/display.rs src/main.rs
git commit -m "perf(list): parallelize worktree status with rayon"
```

---

## Task 8: `--no-cache` CLI Flag

**Files:**
- Modify: `src/cli.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Change `List` variant in `src/cli.rs`**

Replace the `List,` line (around line 221) with:

```rust
    /// List all worktrees
    #[command(alias = "ls")]
    List {
        /// Bypass PR status cache and refresh from gh
        #[arg(long)]
        no_cache: bool,
    },
```

- [ ] **Step 2: Update dispatch in `src/main.rs`**

Replace the match arm at `src/main.rs:53`:

```rust
        Some(Commands::List { no_cache }) => {
            operations::display::list_worktrees(no_cache)?;
        }
```

- [ ] **Step 3: Verify**

Run: `cargo run -- list --help`
Expected output contains `--no-cache  Bypass PR status cache and refresh from gh`.

Run: `cargo run -- list --no-cache`
Expected: runs successfully in the current repo; no panic.

- [ ] **Step 4: Commit**

```bash
git add src/cli.rs src/main.rs
git commit -m "feat(cli): add --no-cache flag to list subcommand"
```

---

## Task 9: TUI Module Skeleton + Style Palette

**Files:**
- Create: `src/tui/mod.rs`
- Create: `src/tui/style.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Create `src/tui/style.rs`**

```rust
//! Shared color palette for ratatui-based views.
//!
//! Mirrors `src/console.rs::status_style` so the TUI and static renderers
//! produce visually identical output.

use ratatui::style::{Color, Modifier, Style};

pub fn status_style(status: &str) -> Style {
    match status {
        "clean" => Style::default().fg(Color::Green),
        "modified" => Style::default().fg(Color::Yellow),
        "busy" => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        "active" => Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        "pr-open" => Style::default().fg(Color::Cyan),
        "merged" => Style::default().fg(Color::Magenta),
        "stale" => Style::default().fg(Color::DarkGray),
        _ => Style::default().add_modifier(Modifier::DIM),
    }
}

pub fn placeholder_style() -> Style {
    Style::default().add_modifier(Modifier::DIM)
}

pub fn header_style() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}
```

- [ ] **Step 2: Create `src/tui/mod.rs`**

```rust
//! TUI rendering layer built on ratatui + crossterm.
//!
//! Used by commands with complex progressive rendering (`gw list` today,
//! potentially interactive/watch commands later). Simple text commands
//! continue to use `crate::console`.

pub mod list_view;
pub mod style;

use std::io::IsTerminal;

/// Whether stdout is attached to a terminal. Commands should fall back to
/// static rendering when this returns false (pipes, redirects, CI).
pub fn stdout_is_tty() -> bool {
    std::io::stdout().is_terminal()
}

/// Install a panic hook that restores the terminal state before the default
/// panic handler prints. Safe to call once at process start.
pub fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = ratatui::restore();
        default(info);
    }));
}
```

- [ ] **Step 3: Register module in `src/lib.rs`**

Open `src/lib.rs` and add `pub mod tui;` alongside the other module declarations.

- [ ] **Step 4: Create placeholder `src/tui/list_view.rs`**

```rust
//! `gw list` Inline Viewport view. Implemented in Task 10.
```

- [ ] **Step 5: Build**

Run: `cargo build`
Expected: builds with zero warnings. (The empty `list_view.rs` is referenced by `mod list_view;` in `mod.rs` — the empty file is valid Rust.)

- [ ] **Step 6: Install panic hook in `src/main.rs`**

At the top of `fn main()` in `src/main.rs`, before any other logic:

```rust
    crate::tui::install_panic_hook();
```

(If `main.rs` references modules via `git_worktree_manager::tui`, use that path instead.)

- [ ] **Step 7: Build & run smoke test**

Run: `cargo run -- list`
Expected: runs successfully, no panic.

- [ ] **Step 8: Commit**

```bash
git add src/tui/mod.rs src/tui/style.rs src/tui/list_view.rs src/lib.rs src/main.rs
git commit -m "feat(tui): add ratatui module skeleton and style palette"
```

---

## Task 10: Inline Viewport List App — Skeleton Render

**Files:**
- Modify: `src/tui/list_view.rs`

- [ ] **Step 1: Write the skeleton-rendering test**

Replace the placeholder `src/tui/list_view.rs` with:

```rust
//! `gw list` Inline Viewport view.

use ratatui::backend::TestBackend;
use ratatui::widgets::{Block, Borders, Cell, Row, Table};
use ratatui::layout::Constraint;
use ratatui::text::Span;
use ratatui::Terminal;

use crate::tui::style;

#[derive(Debug, Clone)]
pub struct RowData {
    pub worktree_id: String,
    pub current_branch: String,
    pub status: String, // "…" while pending
    pub age: String,
    pub rel_path: String,
}

pub struct ListApp {
    pub rows: Vec<RowData>,
}

impl ListApp {
    pub fn new(rows: Vec<RowData>) -> Self {
        Self { rows }
    }

    pub fn is_complete(&self) -> bool {
        self.rows.iter().all(|r| r.status != "…")
    }

    pub fn render(&self, frame: &mut ratatui::Frame<'_>) {
        let header = Row::new(vec![
            Cell::from("WORKTREE"),
            Cell::from("BRANCH"),
            Cell::from("STATUS"),
            Cell::from("AGE"),
            Cell::from("PATH"),
        ])
        .style(style::header_style());

        let body: Vec<Row> = self
            .rows
            .iter()
            .map(|r| {
                let status_cell = if r.status == "…" {
                    Cell::from(Span::styled("…", style::placeholder_style()))
                } else {
                    Cell::from(Span::styled(r.status.clone(), style::status_style(&r.status)))
                };
                Row::new(vec![
                    Cell::from(r.worktree_id.clone()),
                    Cell::from(r.current_branch.clone()),
                    status_cell,
                    Cell::from(r.age.clone()),
                    Cell::from(r.rel_path.clone()),
                ])
            })
            .collect();

        let widths = [
            Constraint::Percentage(20),
            Constraint::Percentage(25),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Percentage(35),
        ];

        let table = Table::new(body, widths)
            .header(header)
            .block(Block::default().borders(Borders::NONE));

        frame.render_widget(table, frame.area());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_row(id: &str, status: &str) -> RowData {
        RowData {
            worktree_id: id.to_string(),
            current_branch: id.to_string(),
            status: status.to_string(),
            age: "1d ago".to_string(),
            rel_path: format!("wt/{}", id),
        }
    }

    #[test]
    fn skeleton_frame_shows_ellipsis_for_all_rows() {
        let app = ListApp::new(vec![
            sample_row("feat/a", "…"),
            sample_row("feat/b", "…"),
        ]);
        let backend = TestBackend::new(80, 6);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| app.render(f)).unwrap();
        let buf = terminal.backend().buffer().clone();
        let rendered = buffer_to_string(&buf);
        assert!(rendered.contains("feat/a"));
        assert!(rendered.contains("feat/b"));
        assert!(rendered.contains("…"));
        assert!(!app.is_complete());
    }

    #[test]
    fn complete_frame_shows_final_status() {
        let app = ListApp::new(vec![
            sample_row("feat/a", "clean"),
            sample_row("feat/b", "modified"),
        ]);
        let backend = TestBackend::new(80, 6);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| app.render(f)).unwrap();
        let buf = terminal.backend().buffer().clone();
        let rendered = buffer_to_string(&buf);
        assert!(rendered.contains("clean"));
        assert!(rendered.contains("modified"));
        assert!(app.is_complete());
    }

    fn buffer_to_string(buf: &ratatui::buffer::Buffer) -> String {
        let mut out = String::new();
        let area = buf.area();
        for y in 0..area.height {
            for x in 0..area.width {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }
}
```

- [ ] **Step 2: Run the tests**

Run: `cargo test -p git-worktree-manager tui::list_view -- --nocapture`
Expected: 2 tests pass.

- [ ] **Step 3: Commit**

```bash
git add src/tui/list_view.rs
git commit -m "feat(tui): Inline Viewport app skeleton with snapshot tests"
```

---

## Task 11: Progressive Render Loop

**Files:**
- Modify: `src/tui/list_view.rs`

- [ ] **Step 1: Add the progressive runner**

Append to `src/tui/list_view.rs` (above the `#[cfg(test)]` block):

```rust
use std::sync::mpsc;

/// Drive the Inline Viewport render loop, consuming `(row_index, status)`
/// updates from `rx` until all rows are filled or the sender disconnects.
///
/// The caller is responsible for spawning the producer (typically a
/// `rayon::spawn` that iterates worktrees in parallel and sends results).
///
/// On return, `app.rows` contains final statuses. The viewport exits via
/// `drop(terminal)` which leaves the final frame in the scrollback.
pub fn run<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut ListApp,
    rx: mpsc::Receiver<(usize, String)>,
) -> std::io::Result<()> {
    terminal.draw(|f| app.render(f))?;

    loop {
        match rx.recv_timeout(std::time::Duration::from_millis(50)) {
            Ok((i, status)) => {
                if let Some(r) = app.rows.get_mut(i) {
                    r.status = status;
                }
                terminal.draw(|f| app.render(f))?;
                if app.is_complete() {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if app.is_complete() {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    Ok(())
}
```

- [ ] **Step 2: Add a test that drives the loop**

Append to the `tests` module in `src/tui/list_view.rs`:

```rust
    #[test]
    fn run_fills_statuses_from_channel() {
        let mut app = ListApp::new(vec![
            sample_row("feat/a", "…"),
            sample_row("feat/b", "…"),
        ]);
        let backend = TestBackend::new(80, 6);
        let mut terminal = Terminal::new(backend).unwrap();

        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            tx.send((0, "clean".to_string())).unwrap();
            tx.send((1, "modified".to_string())).unwrap();
        });

        run(&mut terminal, &mut app, rx).unwrap();
        assert_eq!(app.rows[0].status, "clean");
        assert_eq!(app.rows[1].status, "modified");
        assert!(app.is_complete());
    }
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p git-worktree-manager tui::list_view -- --nocapture`
Expected: 3 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/tui/list_view.rs
git commit -m "feat(tui): progressive render loop consuming mpsc updates"
```

---

## Task 12: Wire TTY Path into `list_worktrees`

**Files:**
- Modify: `src/operations/display.rs`

- [ ] **Step 1: Refactor `list_worktrees` to dispatch TTY vs static**

In `src/operations/display.rs`, replace the parallel collection inside `list_worktrees` (the block that builds `rows` with `par_iter`) with a dispatch. The serial prep of `inputs` remains unchanged. Replace from `let rows: Vec<WorktreeRow> = inputs.par_iter()...` through the footer print with:

```rust
    let rows: Vec<WorktreeRow> = if crate::tui::stdout_is_tty() {
        render_rows_progressive(&repo, &pr_cache, inputs)?
    } else {
        inputs
            .par_iter()
            .map(|i| {
                let status = get_worktree_status(&i.path, &repo, Some(&i.current_branch), &pr_cache);
                WorktreeRow {
                    worktree_id: i.worktree_id.clone(),
                    current_branch: i.current_branch.clone(),
                    status,
                    age: i.age.clone(),
                    rel_path: i.rel_path.clone(),
                }
            })
            .collect()
    };

    // In the TTY path the table has already been rendered inside the
    // Inline Viewport; only print the footer. In the static path, print
    // the table as before.
    if !crate::tui::stdout_is_tty() {
        let term_width = cwconsole::terminal_width();
        if term_width >= MIN_TABLE_WIDTH {
            print_worktree_table(&rows);
        } else {
            print_worktree_compact(&rows);
        }
    }
```

The existing footer block (feature_count + counts + `println!`) stays unchanged and runs for both paths.

- [ ] **Step 2: Add `render_rows_progressive` helper in `display.rs`**

Below `list_worktrees`, add:

```rust
fn render_rows_progressive(
    repo: &Path,
    pr_cache: &crate::operations::pr_cache::PrCache,
    inputs: Vec<RowInput>,
) -> Result<Vec<WorktreeRow>> {
    use crossterm::terminal;
    use ratatui::{backend::CrosstermBackend, Terminal, TerminalOptions, Viewport};
    use std::sync::{mpsc, Arc};

    // Build skeleton app.
    let row_data: Vec<crate::tui::list_view::RowData> = inputs
        .iter()
        .map(|i| crate::tui::list_view::RowData {
            worktree_id: i.worktree_id.clone(),
            current_branch: i.current_branch.clone(),
            status: "…".to_string(),
            age: i.age.clone(),
            rel_path: i.rel_path.clone(),
        })
        .collect();
    let mut app = crate::tui::list_view::ListApp::new(row_data);

    // +2 for header row and one trailing blank line.
    let viewport_height = (inputs.len() as u16).saturating_add(2).max(3);

    let stdout = std::io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::with_options(
        backend,
        TerminalOptions {
            viewport: Viewport::Inline(viewport_height),
        },
    )
    .map_err(|e| crate::error::CwError::Io(e))?;

    // Producer: parallel per-worktree status computation.
    let (tx, rx) = mpsc::channel();
    let repo_arc = Arc::new(repo.to_path_buf());
    let pr_cache_arc = Arc::new(pr_cache.clone());
    let inputs_arc = Arc::new(inputs);
    let producer_inputs = Arc::clone(&inputs_arc);
    let producer_repo = Arc::clone(&repo_arc);
    let producer_cache = Arc::clone(&pr_cache_arc);
    rayon::spawn(move || {
        producer_inputs
            .par_iter()
            .enumerate()
            .for_each_with(tx, |tx, (i, input)| {
                let status = get_worktree_status(
                    &input.path,
                    &producer_repo,
                    Some(&input.current_branch),
                    &producer_cache,
                );
                let _ = tx.send((i, status));
            });
    });

    crate::tui::list_view::run(&mut terminal, &mut app, rx)
        .map_err(|e| crate::error::CwError::Io(e))?;

    // Ensure a final redraw of the complete state, then leave scrollback.
    terminal
        .draw(|f| app.render(f))
        .map_err(|e| crate::error::CwError::Io(e))?;
    drop(terminal);

    // Map RowData → WorktreeRow.
    Ok(app
        .rows
        .into_iter()
        .map(|r| WorktreeRow {
            worktree_id: r.worktree_id,
            current_branch: r.current_branch,
            status: r.status,
            age: r.age,
            rel_path: r.rel_path,
        })
        .collect())
}
```

- [ ] **Step 3: Make `RowInput` and `WorktreeRow` clonable as needed**

`RowInput` must derive `Clone`. Change the struct definition inside `list_worktrees` to a module-level struct (or add `#[derive(Clone)]` if it isn't inline). Move the `struct RowInput { ... }` declaration out of `list_worktrees` to module scope at the top of `display.rs`, next to `WorktreeRow`:

```rust
#[derive(Clone)]
struct RowInput {
    path: std::path::PathBuf,
    current_branch: String,
    worktree_id: String,
    age: String,
    rel_path: String,
}
```

`PrCache` must also be `Clone` (already is, from Task 2).

- [ ] **Step 4: Verify `CwError::Io` variant exists**

Run: `grep -n "Io" /Users/dave/Projects/github.com/git-worktree-manager/src/error.rs | head -5`
Expected: a line like `Io(#[from] std::io::Error),` or similar. If the variant name differs (e.g., `IoError`), adjust the two `.map_err(...)` calls in Step 2 accordingly.

- [ ] **Step 5: Build and test**

Run: `cargo build && cargo test`
Expected: builds with zero warnings, all tests pass.

- [ ] **Step 6: Manual smoke test (TTY)**

Run: `cargo run -- list`
Expected: table appears immediately with `…` in the status column, then statuses fill in. Final table remains in scrollback after the command exits.

- [ ] **Step 7: Manual smoke test (non-TTY)**

Run: `cargo run -- list | cat`
Expected: identical output to the pre-TUI behavior. No escape sequences or `…` placeholders in output.

- [ ] **Step 8: Commit**

```bash
git add src/operations/display.rs
git commit -m "feat(list): progressive Inline Viewport rendering in TTY mode"
```

---

## Task 13: Benchmark and Document

**Files:**
- No code changes. PR description only.

- [ ] **Step 1: Capture before/after timing in a worktree-heavy repo**

In a large repo (e.g. `magicmoment`), run:

```bash
git checkout main
cargo build --release
time ./target/release/gw list >/dev/null
```

Record this number as "before" (it still includes the ParallelIterator + PR cache once committed above; to get the true "before" baseline, stash the changes or compare against `main` before the first commit of this branch).

Then on the feature branch:

```bash
time ./target/release/gw list >/dev/null         # cold cache
time ./target/release/gw list >/dev/null         # warm cache
time ./target/release/gw list --no-cache >/dev/null
```

Record all three.

- [ ] **Step 2: Add results to the PR description**

When opening the PR, include a section:

```markdown
## Benchmark (magicmoment, N worktrees)

- Before: Xs
- After (cold cache): Ys
- After (warm cache): Zs
- After (`--no-cache`): Ws
```

- [ ] **Step 3: No commit required**

(This task is instructional. No code change.)

---

## Summary of Commits (expected order)

1. `chore(deps): add ratatui, crossterm, rayon, sha2 for gw list perf`
2. `feat(pr_cache): add module skeleton with repo-hash and cache path`
3. `feat(pr_cache): fetch PR state from gh in one batched call`
4. `feat(pr_cache): persist PR state to disk with 60s TTL`
5. `feat(pr_cache): public load_or_fetch with --no-cache semantics`
6. `refactor(display): thread PrCache through get_worktree_status`
7. `perf(list): parallelize worktree status with rayon`
8. `feat(cli): add --no-cache flag to list subcommand`
9. `feat(tui): add ratatui module skeleton and style palette`
10. `feat(tui): Inline Viewport app skeleton with snapshot tests`
11. `feat(tui): progressive render loop consuming mpsc updates`
12. `feat(list): progressive Inline Viewport rendering in TTY mode`

---

## Self-Review Notes

**Spec coverage check:**
- A (rayon parallelism): Task 7 (static path) + Task 12 (TTY producer uses `par_iter`). ✓
- B (progressive rendering): Tasks 9–12. ✓
- C (batched gh): Task 3 (`fetch_from_gh` uses `gh pr list`). ✓
- D (disk cache + TTL + `--no-cache`): Tasks 4, 5, 8. ✓
- Edge cases (resize, panic, non-TTY): Task 9 panic hook; non-TTY dispatch in Task 12; resize handled automatically by ratatui Inline Viewport (no explicit task needed). ✓
- Tests for pr_cache: Tasks 2–5. ✓
- Tests for TUI: Tasks 10–11. ✓
- All callers of `get_worktree_status` updated: Task 6. ✓

**Type consistency:**
- `PrCache` → same name everywhere.
- `get_worktree_status(path, repo, branch, &PrCache)` → consistent in Tasks 6, 7, 12.
- `RowData` (in tui::list_view) vs `WorktreeRow` (in display) vs `RowInput` (in display) — three distinct types, each with a clear role. Task 12 maps between them explicitly.
- `run(terminal, app, rx)` signature identical between definition (Task 11) and call site (Task 12).
