/// Core worktree lifecycle operations.
///
/// Mirrors src/claude_worktree/operations/worktree_ops.py (1433 lines).
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use console::style;

use crate::constants::{
    default_worktree_path, format_config_key, CONFIG_KEY_BASE_BRANCH, CONFIG_KEY_BASE_PATH,
    CONFIG_KEY_INTENDED_BRANCH,
};
use crate::error::{CwError, Result};
use crate::git;
use crate::hooks;
use crate::registry;
use crate::shared_files;

use super::helpers::resolve_worktree_target;

/// Create a new worktree with a feature branch.
pub fn create_worktree(
    branch_name: &str,
    base_branch: Option<&str>,
    path: Option<&str>,
    _term: Option<&str>,
    no_ai: bool,
) -> Result<PathBuf> {
    let repo = git::get_repo_root(None)?;

    // Validate branch name
    if !git::is_valid_branch_name(branch_name, Some(&repo)) {
        let error_msg = git::get_branch_name_error(branch_name);
        return Err(CwError::InvalidBranch(format!(
            "Invalid branch name: {}\n\
             Hint: Use alphanumeric characters, hyphens, and slashes.",
            error_msg
        )));
    }

    // Check if worktree already exists
    let existing = git::find_worktree_by_branch(&repo, branch_name)?
        .or(git::find_worktree_by_branch(
            &repo,
            &format!("refs/heads/{}", branch_name),
        )?);

    if let Some(existing_path) = existing {
        println!(
            "\n{}\nBranch '{}' already has a worktree at:\n  {}\n",
            style("! Worktree already exists").yellow().bold(),
            style(branch_name).cyan(),
            style(existing_path.display()).blue(),
        );

        if git::is_non_interactive() {
            return Err(CwError::InvalidBranch(format!(
                "Worktree for branch '{}' already exists at {}.\n\
                 Use 'cw resume {}' to continue work.",
                branch_name,
                existing_path.display(),
                branch_name,
            )));
        }

        // In interactive mode, suggest resume
        println!(
            "Use '{}' to resume work in this worktree.\n",
            style(format!("cw resume {}", branch_name)).cyan()
        );
        return Ok(existing_path);
    }

    // Determine if branch already exists
    let mut branch_already_exists = false;
    let mut is_remote_only = false;

    if git::branch_exists(branch_name, Some(&repo)) {
        println!(
            "\n{}\nBranch '{}' already exists locally but has no worktree.\n",
            style("! Branch already exists").yellow().bold(),
            style(branch_name).cyan(),
        );
        branch_already_exists = true;
    } else if git::remote_branch_exists(branch_name, Some(&repo), "origin") {
        println!(
            "\n{}\nBranch '{}' exists on remote but not locally.\n",
            style("! Remote branch found").yellow().bold(),
            style(branch_name).cyan(),
        );
        branch_already_exists = true;
        is_remote_only = true;
    }

    // Determine base branch
    let base = if is_remote_only && base_branch.is_none() {
        git::get_current_branch(Some(&repo)).unwrap_or_else(|_| "main".to_string())
    } else if let Some(b) = base_branch {
        b.to_string()
    } else {
        git::get_current_branch(Some(&repo)).map_err(|_| {
            CwError::InvalidBranch(
                "Cannot determine base branch. Specify with --branch or checkout a branch first."
                    .to_string(),
            )
        })?
    };

    // Verify base branch
    if (!is_remote_only || base_branch.is_some())
        && !git::branch_exists(&base, Some(&repo))
    {
        return Err(CwError::InvalidBranch(format!(
            "Base branch '{}' not found",
            base
        )));
    }

    // Determine worktree path
    let worktree_path = if let Some(p) = path {
        PathBuf::from(p)
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(p))
    } else {
        default_worktree_path(&repo, branch_name)
    };

    println!("\n{}", style("Creating new worktree:").cyan().bold());
    println!("  Base branch: {}", style(&base).green());
    println!("  New branch:  {}", style(branch_name).green());
    println!("  Path:        {}\n", style(worktree_path.display()).blue());

    // Pre-create hooks
    let mut hook_ctx = HashMap::new();
    hook_ctx.insert("branch".into(), branch_name.to_string());
    hook_ctx.insert("base_branch".into(), base.clone());
    hook_ctx.insert("worktree_path".into(), worktree_path.to_string_lossy().to_string());
    hook_ctx.insert("repo_path".into(), repo.to_string_lossy().to_string());
    hook_ctx.insert("event".into(), "worktree.pre_create".into());
    hook_ctx.insert("operation".into(), "new".into());
    hooks::run_hooks("worktree.pre_create", &hook_ctx, Some(&repo), Some(&repo))?;

    // Create parent dir
    if let Some(parent) = worktree_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    // Fetch
    let _ = git::git_command(&["fetch", "--all", "--prune"], Some(&repo), false, false);

    // Create worktree
    let wt_str = worktree_path.to_string_lossy().to_string();
    if is_remote_only {
        git::git_command(
            &[
                "worktree",
                "add",
                "-b",
                branch_name,
                &wt_str,
                &format!("origin/{}", branch_name),
            ],
            Some(&repo),
            true,
            false,
        )?;
    } else if branch_already_exists {
        git::git_command(
            &["worktree", "add", &wt_str, branch_name],
            Some(&repo),
            true,
            false,
        )?;
    } else {
        git::git_command(
            &["worktree", "add", "-b", branch_name, &wt_str, &base],
            Some(&repo),
            true,
            false,
        )?;
    }

    // Store metadata
    let bb_key = format_config_key(CONFIG_KEY_BASE_BRANCH, branch_name);
    let bp_key = format_config_key(CONFIG_KEY_BASE_PATH, branch_name);
    let ib_key = format_config_key(CONFIG_KEY_INTENDED_BRANCH, branch_name);
    git::set_config(&bb_key, &base, Some(&repo))?;
    git::set_config(&bp_key, &repo.to_string_lossy(), Some(&repo))?;
    git::set_config(&ib_key, branch_name, Some(&repo))?;

    // Register in global registry (non-fatal)
    let _ = registry::register_repo(&repo);

    println!("{} Worktree created successfully\n", style("*").green().bold());

    // Copy shared files
    shared_files::share_files(&repo, &worktree_path);

    // Post-create hooks
    hook_ctx.insert("event".into(), "worktree.post_create".into());
    let _ = hooks::run_hooks("worktree.post_create", &hook_ctx, Some(&worktree_path), Some(&repo));

    // AI tool launch would happen here (Phase 3)
    if !no_ai {
        // TODO: Phase 3 — launch AI tool
    }

    Ok(worktree_path)
}

/// Delete a worktree by branch name, worktree directory name, or path.
pub fn delete_worktree(
    target: Option<&str>,
    keep_branch: bool,
    delete_remote: bool,
) -> Result<()> {
    let main_repo = git::get_main_repo_root(None)?;
    let (worktree_path, branch_name) = resolve_delete_target(target, &main_repo)?;

    // Safety: don't delete main repo
    let wt_resolved = worktree_path
        .canonicalize()
        .unwrap_or_else(|_| worktree_path.clone());
    let main_resolved = main_repo
        .canonicalize()
        .unwrap_or_else(|_| main_repo.clone());
    if wt_resolved == main_resolved {
        return Err(CwError::Git("Cannot delete main repository worktree".to_string()));
    }

    // If cwd is inside worktree, change to main repo
    if let Ok(cwd) = std::env::current_dir() {
        let cwd_str = cwd.to_string_lossy().to_string();
        let wt_str = worktree_path.to_string_lossy().to_string();
        if cwd_str.starts_with(&wt_str) {
            let _ = std::env::set_current_dir(&main_repo);
        }
    }

    // Pre-delete hooks
    let base_branch = branch_name
        .as_deref()
        .and_then(|b| {
            let key = format_config_key(CONFIG_KEY_BASE_BRANCH, b);
            git::get_config(&key, Some(&main_repo))
        })
        .unwrap_or_default();

    let mut hook_ctx = HashMap::new();
    hook_ctx.insert("branch".into(), branch_name.clone().unwrap_or_default());
    hook_ctx.insert("base_branch".into(), base_branch);
    hook_ctx.insert("worktree_path".into(), worktree_path.to_string_lossy().to_string());
    hook_ctx.insert("repo_path".into(), main_repo.to_string_lossy().to_string());
    hook_ctx.insert("event".into(), "worktree.pre_delete".into());
    hook_ctx.insert("operation".into(), "delete".into());
    hooks::run_hooks("worktree.pre_delete", &hook_ctx, Some(&main_repo), Some(&main_repo))?;

    // Remove worktree
    println!("{}", style(format!("Removing worktree: {}", worktree_path.display())).yellow());
    git::remove_worktree_safe(&worktree_path, &main_repo, true)?;
    println!("{} Worktree removed\n", style("*").green().bold());

    // Delete branch
    if let Some(ref branch) = branch_name {
        if !keep_branch {
            println!("{}", style(format!("Deleting local branch: {}", branch)).yellow());
            let _ = git::git_command(
                &["branch", "-D", branch],
                Some(&main_repo),
                false,
                false,
            );

            // Remove metadata
            let bb_key = format_config_key(CONFIG_KEY_BASE_BRANCH, branch);
            let bp_key = format_config_key(CONFIG_KEY_BASE_PATH, branch);
            let ib_key = format_config_key(CONFIG_KEY_INTENDED_BRANCH, branch);
            git::unset_config(&bb_key, Some(&main_repo));
            git::unset_config(&bp_key, Some(&main_repo));
            git::unset_config(&ib_key, Some(&main_repo));

            println!("{} Local branch and metadata removed\n", style("*").green().bold());

            // Delete remote branch
            if delete_remote {
                println!(
                    "{}",
                    style(format!("Deleting remote branch: origin/{}", branch)).yellow()
                );
                match git::git_command(
                    &["push", "origin", &format!(":{}", branch)],
                    Some(&main_repo),
                    false,
                    true,
                ) {
                    Ok(r) if r.returncode == 0 => {
                        println!("{} Remote branch deleted\n", style("*").green().bold());
                    }
                    _ => {
                        println!("{} Remote branch deletion failed\n", style("!").yellow());
                    }
                }
            }
        }
    }

    // Post-delete hooks
    hook_ctx.insert("event".into(), "worktree.post_delete".into());
    let _ = hooks::run_hooks("worktree.post_delete", &hook_ctx, Some(&main_repo), Some(&main_repo));
    let _ = registry::update_last_seen(&main_repo);

    Ok(())
}

/// Resolve delete target to (worktree_path, branch_name).
fn resolve_delete_target(
    target: Option<&str>,
    main_repo: &Path,
) -> Result<(PathBuf, Option<String>)> {
    let target = target
        .map(|t| t.to_string())
        .unwrap_or_else(|| {
            std::env::current_dir()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        });

    let target_path = PathBuf::from(&target);

    // Check if it's a filesystem path
    if target_path.exists() {
        let resolved = target_path.canonicalize().unwrap_or(target_path);
        let branch = super::helpers::get_branch_for_worktree(main_repo, &resolved);
        return Ok((resolved, branch));
    }

    // Try branch lookup
    if let Some(path) = git::find_worktree_by_intended_branch(main_repo, &target)? {
        return Ok((path, Some(target)));
    }

    // Try worktree name lookup
    if let Some(path) = git::find_worktree_by_name(main_repo, &target)? {
        let branch = super::helpers::get_branch_for_worktree(main_repo, &path);
        return Ok((path, branch));
    }

    Err(CwError::WorktreeNotFound(format!(
        "No worktree found for '{}'. Try: full path, branch name, or worktree name.",
        target
    )))
}

/// Sync worktree with base branch.
pub fn sync_worktree(
    target: Option<&str>,
    all: bool,
    _fetch_only: bool,
) -> Result<()> {
    let repo = git::get_repo_root(None)?;

    // Fetch first
    println!("{}", style("Fetching updates from remote...").yellow());
    let fetch_result = git::git_command(
        &["fetch", "--all", "--prune"],
        Some(&repo),
        false,
        true,
    )?;
    if fetch_result.returncode != 0 {
        println!("{} Fetch failed or no remote configured\n", style("!").yellow());
    }

    if _fetch_only {
        println!("{} Fetch complete\n", style("*").green().bold());
        return Ok(());
    }

    // Determine worktrees to sync
    let worktrees_to_sync = if all {
        let all_wt = git::parse_worktrees(&repo)?;
        all_wt
            .into_iter()
            .filter(|(b, _)| b != "(detached)")
            .map(|(b, p)| {
                let branch = git::normalize_branch_name(&b).to_string();
                (branch, p)
            })
            .collect::<Vec<_>>()
    } else {
        let (path, branch, _) = resolve_worktree_target(target, None)?;
        vec![(branch, path)]
    };

    for (branch, wt_path) in &worktrees_to_sync {
        let base_key = format_config_key(CONFIG_KEY_BASE_BRANCH, branch);
        let base_branch = git::get_config(&base_key, Some(&repo));

        if let Some(base) = base_branch {
            println!("\n{}", style("Syncing worktree:").cyan().bold());
            println!("  Branch: {}", style(branch).green());
            println!("  Base:   {}", style(&base).green());
            println!("  Path:   {}\n", style(wt_path.display()).blue());

            // Determine rebase target
            let rebase_target = {
                let origin_base = format!("origin/{}", base);
                if git::branch_exists(&origin_base, Some(wt_path)) {
                    origin_base
                } else {
                    base.clone()
                }
            };

            println!(
                "{}",
                style(format!("Rebasing {} onto {}...", branch, rebase_target)).yellow()
            );

            match git::git_command(&["rebase", &rebase_target], Some(wt_path), false, true) {
                Ok(r) if r.returncode == 0 => {
                    println!("{} Rebase successful\n", style("*").green().bold());
                }
                _ => {
                    // Abort rebase on failure
                    let _ = git::git_command(
                        &["rebase", "--abort"],
                        Some(wt_path),
                        false,
                        false,
                    );
                    println!(
                        "{} Rebase failed for '{}'. Resolve conflicts manually.\n",
                        style("!").yellow(),
                        branch
                    );
                }
            }
        } else {
            // No base branch metadata — try origin/branch
            let origin_ref = format!("origin/{}", branch);
            if git::branch_exists(&origin_ref, Some(wt_path)) {
                println!("\n{}", style("Syncing worktree:").cyan().bold());
                println!("  Branch: {}", style(branch).green());
                println!("  Path:   {}\n", style(wt_path.display()).blue());

                println!(
                    "{}",
                    style(format!("Rebasing {} onto {}...", branch, origin_ref)).yellow()
                );

                match git::git_command(&["rebase", &origin_ref], Some(wt_path), false, true) {
                    Ok(r) if r.returncode == 0 => {
                        println!("{} Rebase successful\n", style("*").green().bold());
                    }
                    _ => {
                        let _ = git::git_command(
                            &["rebase", "--abort"],
                            Some(wt_path),
                            false,
                            false,
                        );
                        println!(
                            "{} Rebase failed for '{}'. Resolve conflicts manually.\n",
                            style("!").yellow(),
                            branch
                        );
                    }
                }
            }
        }
    }

    Ok(())
}
