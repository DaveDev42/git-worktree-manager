/// Batch cleanup of worktrees.
///
use console::style;
use std::path::{Path, PathBuf};

use crate::constants::{format_config_key, path_age_days, CONFIG_KEY_BASE_BRANCH};
use crate::error::Result;
use crate::git;
use crate::messages;
use crate::operations::delete_batch;
use crate::operations::worktree::DeleteFlags;

use super::pr_cache::{PrCache, PrState};

/// Determine whether `branch` is merged, using the same two-step logic as
/// `gw list`:
///   1. PrCache (primary) — squash-merge aware, checks GitHub PR state.
///   2. `git branch --merged` (fallback) — only catches traditional merge
///      commits, but still useful when `gh` is not available.
///
/// Returns `Some(base_branch_name)` if merged, `None` otherwise. Returning
/// the resolved base lets the caller render an accurate "merged into <base>"
/// reason without re-deriving the base separately (which would risk silent
/// drift if the resolution logic ever changed in only one place).
///
/// The base branch is read from `branch.<name>.worktreeBase` git config; if
/// absent, `git::detect_default_branch` is used as the fallback.  The old
/// code silently skipped the merged check when the config key was missing,
/// which caused `gw clean --merged` to miss every squash-merged branch (the
/// live bug: `gw list` showed "merged" while `gw clean --merged` said "No
/// worktrees match").
pub(super) fn branch_is_merged(
    branch_name: &str,
    repo: &Path,
    pr_cache: &PrCache,
) -> Option<String> {
    // Determine base branch: git config first, repo default second.
    let base_key = format_config_key(CONFIG_KEY_BASE_BRANCH, branch_name);
    let base_branch = git::get_config(&base_key, Some(repo))
        .unwrap_or_else(|| git::detect_default_branch(Some(repo)));

    // Primary: cached GitHub PR state (squash-merge aware).
    if matches!(pr_cache.state(branch_name), Some(PrState::Merged)) {
        return Some(base_branch);
    }

    // Fallback: git branch --merged (traditional merge commits only).
    if git::is_branch_merged(branch_name, &base_branch, Some(repo)) {
        return Some(base_branch);
    }

    None
}

/// Top-level `gw clean` entry point.
///
/// Returns an exit code (0/1/2) matching the convention `gw delete` uses:
/// - `0`: every selected worktree was deleted (or filters matched nothing).
/// - `1`: user cancelled at the batch-confirmation prompt.
/// - `2`: misuse (no filter flags / removed `-i` flag) or partial failure.
pub fn clean_worktrees(
    merged: bool,
    older_than: Option<u64>,
    dry_run: bool,
    force: bool,
    interactive: bool,
) -> Result<i32> {
    if interactive {
        eprintln!(
            "Error: `gw clean -i` has been removed.\n\
             For interactive selection, use `gw delete -i` instead."
        );
        return Ok(2);
    }

    if !merged && older_than.is_none() {
        eprintln!(
            "Error: `gw clean` requires at least one filter:\n  \
             --merged, --older-than <DURATION>.\n\
             For interactive selection, use `gw delete -i`."
        );
        return Ok(2);
    }

    let main_repo = git::get_repo_root(None)?;
    let candidates = git::get_feature_worktrees(Some(&main_repo))?;

    let pr_cache = PrCache::load_or_fetch(&main_repo, false);

    let is_merged_pred =
        |branch: &str| -> bool { branch_is_merged(branch, &main_repo, &pr_cache).is_some() };
    let is_old_pred = |path: &Path| -> bool {
        match older_than {
            Some(days) => path_age_days(path)
                .map(|age| age as u64 >= days)
                .unwrap_or(false),
            None => false,
        }
    };
    let selected = filter_worktrees_pure(
        &candidates,
        merged,
        older_than,
        &is_merged_pred,
        &is_old_pred,
    );

    if selected.is_empty() {
        println!(
            "{} No worktrees match the cleanup criteria",
            style("*").green().bold()
        );
        return Ok(0);
    }

    // Hand the selection to the shared deletion engine. The `DeleteFlags`
    // here mirror the legacy `clean` semantics:
    // - keep_branch=false / delete_remote=false: clean removes the local
    //   branch metadata only.
    // - git_force=true: matches the historical `git worktree remove --force`
    //   behavior — the worktree may have local-only state.
    // - allow_busy=force: only `--force` lets clean tear through a busy
    //   worktree (one held by `gw shell` / claude / editor).
    let flags = DeleteFlags {
        keep_branch: false,
        delete_remote: false,
        git_force: true,
        allow_busy: force,
    };
    let code = delete_batch::delete_worktrees(
        selected.clone(),
        /*interactive=*/ false,
        dry_run,
        flags,
        /*lookup_mode=*/ None,
    )?;

    // Prune stale metadata on any non-cancelled path (matches legacy UX).
    // If the user cancelled at the batch prompt (code == 1) nothing was
    // deleted, so there's no metadata to prune.
    if code != 1 {
        println!("{}", style("Pruning stale worktree metadata...").dim());
        let _ = git::git_command(&["worktree", "prune"], Some(&main_repo), false, false);
        println!("{}", style("* Prune complete").dim());
    }

    // On partial failure `delete_batch` already printed a Summary line, so
    // only print the success banner when every selection succeeded.
    if code == 0 && !dry_run {
        let count = selected.len() as u32;
        println!(
            "\n{}",
            style(messages::cleanup_complete(count)).green().bold()
        );
    }

    Ok(code)
}

/// Pure selection helper. Takes a list of `(branch, path)` candidates and
/// the active filter predicates; returns branch names matching at least one
/// active filter (UNION semantics — a worktree matching either `--merged` OR
/// `--older-than` qualifies, mirroring the legacy behavior). Generic over
/// the predicates so unit tests can inject deterministic ones without a
/// real git repo or PrCache.
pub(crate) fn filter_worktrees_pure(
    candidates: &[(String, PathBuf)],
    merged: bool,
    older_than_days: Option<u64>,
    is_merged: &dyn Fn(&str) -> bool,
    is_old: &dyn Fn(&Path) -> bool,
) -> Vec<String> {
    if !merged && older_than_days.is_none() {
        return Vec::new();
    }
    candidates
        .iter()
        .filter(|(branch, path)| {
            let m = merged && is_merged(branch);
            let o = older_than_days.is_some() && is_old(path);
            m || o
        })
        .map(|(branch, _)| branch.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    // The env-var lock and guard are defined in `super::super::test_env` so
    // this module and `pr_cache` share a single mutex. Without sharing, each
    // module's tests would hold a different lock and race on the same global
    // env vars (`GW_TEST_GH_JSON`, `GW_TEST_GH_FAIL`, `GW_TEST_CACHE_DIR`).
    use super::super::test_env::{env_lock, EnvGuard};

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
            result.is_some(),
            "branch_is_merged must return Some(base) when PrCache reports MERGED, \
             even without a worktreeBase git config entry (the live bug)"
        );
    }

    // ──────────────────────────────────────────────────────────────────────
    // Case C: branch with no PR, not reachable from base, no worktreeBase.
    //         Predicate must return None (no false-positive).
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
            result.is_none(),
            "branch_is_merged must return None for an unmerged branch with no PR \
             and no worktreeBase config"
        );
    }

    // ──────────────────────────────────────────────────────────────────────
    // The reason string returned to the user must agree with the resolved
    // base branch — i.e., the helper returns the SAME base it used for the
    // git fallback. Pins single-source-of-truth so the "merged into <base>"
    // reason cannot silently disagree with the predicate's actual base.
    // ──────────────────────────────────────────────────────────────────────
    #[test]
    fn reason_base_matches_resolved_worktree_base_config() {
        let _g = env_lock();
        let _env = EnvGuard::capture(&["GW_TEST_GH_JSON", "GW_TEST_GH_FAIL", "GW_TEST_CACHE_DIR"]);

        std::env::set_var(
            "GW_TEST_GH_JSON",
            r#"[{"headRefName":"some-feature","state":"MERGED"}]"#,
        );
        let tmp_repo =
            std::path::PathBuf::from(format!("/tmp/gw-test-unit-reason-{}", std::process::id()));
        let cache = PrCache::load_or_fetch(&tmp_repo, true);

        let repo_dir = tempfile::tempdir().unwrap();
        let repo = repo_dir.path();
        init_git_repo(repo);

        // Set worktreeBase to a non-default value so we can tell the helper
        // honored it (instead of silently falling back to detect_default_branch).
        std::process::Command::new("git")
            .args(["config", "branch.some-feature.worktreeBase", "develop"])
            .current_dir(repo)
            .output()
            .unwrap();

        let result = branch_is_merged("some-feature", repo, &cache);
        assert_eq!(
            result.as_deref(),
            Some("develop"),
            "branch_is_merged must return the worktreeBase config value as the \
             resolved base, so the user-facing 'merged into <base>' reason \
             cannot drift from what the predicate actually checked"
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
        assert!(result.is_none(), "An OPEN PR must not be considered merged");
    }

    #[test]
    fn filter_worktrees_pure_selects_union_of_matching_rules() {
        let candidates: Vec<(String, std::path::PathBuf)> = vec![
            (
                "feat/merged-only".into(),
                std::path::PathBuf::from("/tmp/a"),
            ),
            ("feat/old-only".into(), std::path::PathBuf::from("/tmp/b")),
            ("feat/both".into(), std::path::PathBuf::from("/tmp/c")),
            ("feat/neither".into(), std::path::PathBuf::from("/tmp/d")),
        ];
        let is_merged =
            |branch: &str| -> bool { matches!(branch, "feat/merged-only" | "feat/both") };
        let is_old = |path: &std::path::Path| -> bool {
            matches!(path.to_str().unwrap_or(""), "/tmp/b" | "/tmp/c")
        };
        let selected = super::filter_worktrees_pure(
            &candidates,
            /*merged=*/ true,
            /*older_than_days=*/ Some(30),
            &is_merged,
            &is_old,
        );
        assert_eq!(
            selected,
            vec![
                "feat/merged-only".to_string(),
                "feat/old-only".to_string(),
                "feat/both".to_string(),
            ]
        );
    }

    #[test]
    fn filter_worktrees_pure_empty_when_no_filters_active() {
        let candidates: Vec<(String, std::path::PathBuf)> =
            vec![("x".into(), std::path::PathBuf::from("/tmp/x"))];
        let is_merged = |_: &str| true;
        let is_old = |_: &std::path::Path| true;
        let selected = super::filter_worktrees_pure(
            &candidates,
            /*merged=*/ false,
            /*older_than_days=*/ None,
            &is_merged,
            &is_old,
        );
        assert!(selected.is_empty());
    }
}
