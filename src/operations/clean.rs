/// Batch cleanup of worktrees.
///
use console::style;
use std::path::Path;

use crate::constants::{format_config_key, path_age_days, CONFIG_KEY_BASE_BRANCH};
use crate::error::Result;
use crate::git;
use crate::messages;

use super::display::get_worktree_status;
use super::pr_cache::{PrCache, PrState};

/// Determine whether `branch` is merged, using the same two-step logic as
/// `gw list`:
///   1. PrCache (primary) — squash-merge aware, checks GitHub PR state.
///   2. `git branch --merged` (fallback) — only catches traditional merge
///      commits, but still useful when `gh` is not available.
///
/// The base branch is read from `branch.<name>.worktreeBase` git config; if
/// absent, `git::detect_default_branch` is used as the fallback.  The old
/// code silently skipped the merged check when the config key was missing,
/// which caused `gw clean --merged` to miss every squash-merged branch (the
/// live bug: `gw list` showed "merged" while `gw clean --merged` said "No
/// worktrees match").
///
/// Visibility is `pub(crate)` — the helper is tested via unit tests in this
/// module (`#[cfg(test)] mod tests`), not from external integration tests.
pub(crate) fn branch_is_merged(branch_name: &str, repo: &Path, pr_cache: &PrCache) -> bool {
    // Determine base branch: git config first, repo default second.
    let base_key = format_config_key(CONFIG_KEY_BASE_BRANCH, branch_name);
    let base_branch = git::get_config(&base_key, Some(repo))
        .unwrap_or_else(|| git::detect_default_branch(Some(repo)));

    // Primary: cached GitHub PR state (squash-merge aware).
    if matches!(pr_cache.state(branch_name), Some(PrState::Merged)) {
        return true;
    }

    // Fallback: git branch --merged (traditional merge commits only).
    git::is_branch_merged(branch_name, &base_branch, Some(repo))
}

/// Batch cleanup of worktrees based on criteria.
pub fn clean_worktrees(
    no_cache: bool,
    merged: bool,
    older_than: Option<u64>,
    interactive: bool,
    dry_run: bool,
    force: bool,
) -> Result<()> {
    let repo = git::get_repo_root(None)?;

    // Must specify at least one criterion
    if !merged && older_than.is_none() && !interactive {
        eprintln!(
            "Error: Please specify at least one cleanup criterion:\n  \
             --merged, --older-than, or -i/--interactive"
        );
        return Ok(());
    }

    // Load the PR cache once at the top so the merged-check and the interactive
    // listing both share the same instance (no double fetch).
    let pr_cache = PrCache::load_or_fetch(&repo, no_cache);

    let mut to_delete: Vec<(String, String, String)> = Vec::new(); // (branch, path, reason)

    for (branch_name, path) in git::get_feature_worktrees(Some(&repo))? {
        let mut should_delete = false;
        let mut reasons = Vec::new();

        // Check if merged — mirrors `gw list`'s merge-detection strategy:
        // PrCache first (squash-merge aware), git fallback second.
        if merged {
            let base_key = format_config_key(CONFIG_KEY_BASE_BRANCH, &branch_name);
            let base_branch = git::get_config(&base_key, Some(&repo))
                .unwrap_or_else(|| git::detect_default_branch(Some(&repo)));

            if branch_is_merged(&branch_name, &repo, &pr_cache) {
                should_delete = true;
                reasons.push(format!("merged into {}", base_branch));
            }
        }

        // Check age
        if let Some(days) = older_than {
            if let Some(age) = path_age_days(&path) {
                let age_days = age as u64;
                if age_days >= days {
                    should_delete = true;
                    reasons.push(format!("older than {} days ({} days)", days, age_days));
                }
            }
        }

        if should_delete {
            to_delete.push((
                branch_name.clone(),
                path.to_string_lossy().to_string(),
                reasons.join(", "),
            ));
        }
    }

    // Interactive mode
    if interactive && to_delete.is_empty() {
        println!("{}\n", style("Available worktrees:").cyan().bold());
        let mut all_wt = Vec::new();
        // Reuse the already-loaded pr_cache instance (no second fetch).
        for (branch_name, path) in git::get_feature_worktrees(Some(&repo))? {
            let status = get_worktree_status(&path, &repo, Some(branch_name.as_str()), &pr_cache);
            println!("  [{:8}] {:<30} {}", status, branch_name, path.display());
            all_wt.push((branch_name, path.to_string_lossy().to_string()));
        }
        println!();
        println!("Enter branch names to delete (space-separated), or 'all' for all:");

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.eq_ignore_ascii_case("all") {
            to_delete = all_wt
                .into_iter()
                .map(|(b, p)| (b, p, "user selected".to_string()))
                .collect();
        } else {
            let selected: Vec<&str> = input.split_whitespace().collect();
            to_delete = all_wt
                .into_iter()
                .filter(|(b, _)| selected.contains(&b.as_str()))
                .map(|(b, p)| (b, p, "user selected".to_string()))
                .collect();
        }

        if to_delete.is_empty() {
            println!("{}", style("No worktrees selected for deletion").yellow());
            return Ok(());
        }
    }

    // Skip worktrees that another session is actively using, unless --force.
    // This prevents `gw clean --merged` from wiping a worktree held open by
    // a Claude Code / shell / editor session. Users can pass --force to
    // ignore the busy gate.
    let mut busy_skipped: Vec<(
        String,
        Vec<crate::operations::busy::BusyInfo>,
        Vec<crate::operations::busy::BusyInfo>,
    )> = Vec::new();
    if !force {
        let mut kept: Vec<(String, String, String)> = Vec::with_capacity(to_delete.len());
        for (branch, path, reason) in to_delete.into_iter() {
            let (hard, soft) =
                crate::operations::busy::detect_busy_tiered(std::path::Path::new(&path));
            if hard.is_empty() && soft.is_empty() {
                kept.push((branch, path, reason));
            } else {
                busy_skipped.push((branch, hard, soft));
            }
        }
        to_delete = kept;
    }

    if !busy_skipped.is_empty() {
        println!(
            "{}",
            style(format!(
                "Skipping {} busy worktree(s) (use --force to override):",
                busy_skipped.len()
            ))
            .yellow()
        );
        for (branch, hard, soft) in &busy_skipped {
            eprint!(
                "{}",
                crate::operations::busy_messages::render_refusal(branch, hard, soft)
            );
        }
        println!();
    }

    if to_delete.is_empty() {
        println!(
            "{} No worktrees match the cleanup criteria\n",
            style("*").green().bold()
        );
        return Ok(());
    }

    // Show what will be deleted
    let prefix = if dry_run { "DRY RUN: " } else { "" };
    println!(
        "\n{}\n",
        style(format!("{}Worktrees to delete:", prefix))
            .yellow()
            .bold()
    );
    for (branch, path, reason) in &to_delete {
        println!("  - {:<30} ({})", branch, reason);
        println!("    Path: {}", path);
    }
    println!();

    if dry_run {
        println!(
            "{} Would delete {} worktree(s)",
            style("*").cyan().bold(),
            to_delete.len()
        );
        println!("Run without --dry-run to actually delete them");
        return Ok(());
    }

    // Delete worktrees
    let mut deleted = 0u32;
    for (branch, _, _) in &to_delete {
        println!("{}", style(format!("Deleting {}...", branch)).yellow());
        // clean already filtered out busy worktrees above (unless --force),
        // so at this point we pass allow_busy=true to skip the redundant
        // gate inside delete_worktree.
        match super::worktree::delete_worktree(Some(branch), false, false, true, true, None) {
            Ok(()) => {
                println!("{} Deleted {}", style("*").green().bold(), branch);
                deleted += 1;
            }
            Err(e) => {
                println!(
                    "{} Failed to delete {}: {}",
                    style("x").red().bold(),
                    branch,
                    e
                );
            }
        }
    }

    println!(
        "\n{}\n",
        style(messages::cleanup_complete(deleted)).green().bold()
    );

    // Prune stale metadata
    println!("{}", style("Pruning stale worktree metadata...").dim());
    let _ = git::git_command(&["worktree", "prune"], Some(&repo), false, false);
    println!("{}\n", style("* Prune complete").dim());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard};

    /// Serialise env-var mutations: `fetch_from_gh` / `cache_path_for` in
    /// `pr_cache.rs` read `GW_TEST_GH_JSON`, `GW_TEST_GH_FAIL`, and
    /// `GW_TEST_CACHE_DIR` — all gated on `#[cfg(test)]` so they only fire
    /// in unit-test mode (which is why these tests live here, not in
    /// `tests/test_clean_merged.rs`).
    static ENV_LOCK: Mutex<()> = Mutex::new(());
    fn env_lock() -> MutexGuard<'static, ()> {
        ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    struct EnvGuard {
        saved: Vec<(&'static str, Option<std::ffi::OsString>)>,
    }
    impl EnvGuard {
        fn capture(keys: &[&'static str]) -> Self {
            let saved = keys.iter().map(|k| (*k, std::env::var_os(k))).collect();
            Self { saved }
        }
    }
    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (k, v) in self.saved.drain(..) {
                match v {
                    Some(val) => std::env::set_var(k, val),
                    None => std::env::remove_var(k),
                }
            }
        }
    }

    fn init_git_repo(path: &std::path::Path) {
        for args in &[
            vec!["init", "-b", "main"],
            vec!["config", "user.name", "Test"],
            vec!["config", "user.email", "test@test.com"],
            vec!["config", "commit.gpgsign", "false"],
        ] {
            std::process::Command::new("git")
                .args(args)
                .current_dir(path)
                .output()
                .unwrap();
        }
    }

    // ──────────────────────────────────────────────────────────────────────
    // Case A: squash-merged branch detected via PrCache, worktreeBase MISSING.
    //
    // This is the live bug: `gw list` shows "merged", but the old
    // `gw clean --merged` skipped the check entirely when worktreeBase
    // was absent from git config.
    // ──────────────────────────────────────────────────────────────────────
    #[test]
    fn case_a_squash_merged_pr_cache_no_worktree_base() {
        let _g = env_lock();
        let _env = EnvGuard::capture(&["GW_TEST_GH_JSON", "GW_TEST_GH_FAIL", "GW_TEST_CACHE_DIR"]);

        // Inject a MERGED PR into the PrCache via the test env hook.
        std::env::set_var(
            "GW_TEST_GH_JSON",
            r#"[{"headRefName":"fix-squash-branch","state":"MERGED"}]"#,
        );
        let tmp_repo =
            std::path::PathBuf::from(format!("/tmp/gw-test-unit-a-{}", std::process::id()));
        let cache = PrCache::load_or_fetch(&tmp_repo, true);

        // Sanity: ensure the cache has the MERGED state before calling the predicate.
        assert_eq!(
            cache.state("fix-squash-branch"),
            Some(&super::super::pr_cache::PrState::Merged),
            "PrCache must report Merged for the test to be meaningful"
        );

        // Use a real tempdir as "repo" — worktreeBase is intentionally absent.
        let repo_dir = tempfile::tempdir().unwrap();
        let repo = repo_dir.path();
        init_git_repo(repo);

        let result = branch_is_merged("fix-squash-branch", repo, &cache);
        assert!(
            result,
            "branch_is_merged must return true when PrCache reports MERGED, \
             even without a worktreeBase git config entry (the live bug)"
        );
    }

    // ──────────────────────────────────────────────────────────────────────
    // Case C: branch with no PR, not reachable from base, no worktreeBase.
    //         Predicate must return false (no false-positive).
    // ──────────────────────────────────────────────────────────────────────
    #[test]
    fn case_c_no_pr_not_merged_no_worktree_base() {
        let _g = env_lock();
        let _env = EnvGuard::capture(&["GW_TEST_GH_JSON", "GW_TEST_GH_FAIL", "GW_TEST_CACHE_DIR"]);

        // Empty cache — no PRs at all.
        std::env::set_var("GW_TEST_GH_FAIL", "1");
        let tmp_repo =
            std::path::PathBuf::from(format!("/tmp/gw-test-unit-c-{}", std::process::id()));
        let cache = PrCache::load_or_fetch(&tmp_repo, true);

        let repo_dir = tempfile::tempdir().unwrap();
        let repo = repo_dir.path();
        init_git_repo(repo);

        // Initial commit on main
        std::fs::write(repo.join("README.md"), "hi").unwrap();
        for args in &[vec!["add", "."], vec!["commit", "-m", "init"]] {
            std::process::Command::new("git")
                .args(args)
                .current_dir(repo)
                .env("GIT_AUTHOR_NAME", "Test")
                .env("GIT_AUTHOR_EMAIL", "test@test.com")
                .env("GIT_COMMITTER_NAME", "Test")
                .env("GIT_COMMITTER_EMAIL", "test@test.com")
                .output()
                .unwrap();
        }
        // Unmerged feature branch
        std::process::Command::new("git")
            .args(["checkout", "-b", "feat-unmerged"])
            .current_dir(repo)
            .output()
            .unwrap();
        std::fs::write(repo.join("feat.txt"), "work").unwrap();
        for args in &[vec!["add", "."], vec!["commit", "-m", "feat work"]] {
            std::process::Command::new("git")
                .args(args)
                .current_dir(repo)
                .env("GIT_AUTHOR_NAME", "Test")
                .env("GIT_AUTHOR_EMAIL", "test@test.com")
                .env("GIT_COMMITTER_NAME", "Test")
                .env("GIT_COMMITTER_EMAIL", "test@test.com")
                .output()
                .unwrap();
        }

        let result = branch_is_merged("feat-unmerged", repo, &cache);
        assert!(
            !result,
            "branch_is_merged must return false for an unmerged branch with no PR \
             and no worktreeBase config"
        );
    }

    // ──────────────────────────────────────────────────────────────────────
    // Regression: PrCache MERGED always wins regardless of worktreeBase.
    // ──────────────────────────────────────────────────────────────────────
    #[test]
    fn regression_pr_cache_merged_wins_over_missing_worktree_base() {
        let _g = env_lock();
        let _env = EnvGuard::capture(&["GW_TEST_GH_JSON", "GW_TEST_GH_FAIL", "GW_TEST_CACHE_DIR"]);

        std::env::set_var(
            "GW_TEST_GH_JSON",
            r#"[{"headRefName":"some-feature","state":"MERGED"}]"#,
        );
        let tmp_repo =
            std::path::PathBuf::from(format!("/tmp/gw-test-unit-reg-{}", std::process::id()));
        let cache = PrCache::load_or_fetch(&tmp_repo, true);

        let repo_dir = tempfile::tempdir().unwrap();
        let repo = repo_dir.path();
        init_git_repo(repo);

        // worktreeBase config is intentionally absent.
        let result = branch_is_merged("some-feature", repo, &cache);
        assert!(
            result,
            "PrCache MERGED state must cause branch_is_merged to return true \
             regardless of whether worktreeBase config is present"
        );
    }

    // ──────────────────────────────────────────────────────────────────────
    // PrCache OPEN must not mark a branch merged.
    // ──────────────────────────────────────────────────────────────────────
    #[test]
    fn pr_open_is_not_merged() {
        let _g = env_lock();
        let _env = EnvGuard::capture(&["GW_TEST_GH_JSON", "GW_TEST_GH_FAIL", "GW_TEST_CACHE_DIR"]);

        std::env::set_var(
            "GW_TEST_GH_JSON",
            r#"[{"headRefName":"feat-open","state":"OPEN"}]"#,
        );
        let tmp_repo =
            std::path::PathBuf::from(format!("/tmp/gw-test-unit-open-{}", std::process::id()));
        let cache = PrCache::load_or_fetch(&tmp_repo, true);

        let repo_dir = tempfile::tempdir().unwrap();
        let repo = repo_dir.path();
        init_git_repo(repo);

        let result = branch_is_merged("feat-open", repo, &cache);
        assert!(!result, "An OPEN PR must not be considered merged");
    }
}
