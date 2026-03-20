/// Git operations for pull requests and merging.
///
/// Mirrors src/git_worktree_manager/operations/git_ops.py (412 lines).
use std::collections::HashMap;
use std::process::Command;

use console::style;

use crate::constants::{
    format_config_key, CONFIG_KEY_BASE_BRANCH, CONFIG_KEY_BASE_PATH, CONFIG_KEY_INTENDED_BRANCH,
};
use crate::error::{CwError, Result};
use crate::git;
use crate::hooks;
use crate::registry;

use super::helpers::{get_worktree_metadata, resolve_worktree_target};
use crate::messages;

/// Create a GitHub Pull Request for the worktree.
pub fn create_pr_worktree(
    target: Option<&str>,
    push: bool,
    title: Option<&str>,
    body: Option<&str>,
    draft: bool,
    lookup_mode: Option<&str>,
) -> Result<()> {
    if !git::has_command("gh") {
        return Err(CwError::Git(messages::gh_cli_not_found()));
    }

    let (cwd, feature_branch, worktree_repo) = resolve_worktree_target(target, lookup_mode)?;
    let (base_branch, base_path) = get_worktree_metadata(&feature_branch, &worktree_repo)?;

    println!("\n{}", style("Creating Pull Request:").cyan().bold());
    println!("  Feature:     {}", style(&feature_branch).green());
    println!("  Base:        {}", style(&base_branch).green());
    println!("  Repo:        {}\n", style(base_path.display()).blue());

    // Pre-PR hooks
    let mut hook_ctx = HashMap::new();
    hook_ctx.insert("branch".into(), feature_branch.clone());
    hook_ctx.insert("base_branch".into(), base_branch.clone());
    hook_ctx.insert("worktree_path".into(), cwd.to_string_lossy().to_string());
    hook_ctx.insert("repo_path".into(), base_path.to_string_lossy().to_string());
    hook_ctx.insert("event".into(), "pr.pre".into());
    hook_ctx.insert("operation".into(), "pr".into());
    hooks::run_hooks("pr.pre", &hook_ctx, Some(&cwd), Some(&base_path))?;

    // Fetch
    println!("{}", style("Fetching updates from remote...").yellow());
    let fetch_ok = git::git_command(
        &["fetch", "--all", "--prune"],
        Some(&base_path),
        false,
        true,
    )
    .map(|r| r.returncode == 0)
    .unwrap_or(false);

    // Determine rebase target
    let rebase_target = if fetch_ok {
        let origin_ref = format!("origin/{}", base_branch);
        if git::branch_exists(&origin_ref, Some(&cwd)) {
            origin_ref
        } else {
            base_branch.clone()
        }
    } else {
        base_branch.clone()
    };

    // Rebase
    println!(
        "{}",
        style(format!(
            "Rebasing {} onto {}...",
            feature_branch, rebase_target
        ))
        .yellow()
    );

    match git::git_command(&["rebase", &rebase_target], Some(&cwd), false, true) {
        Ok(r) if r.returncode == 0 => {}
        _ => {
            // Abort and report
            let conflicts = git::git_command(
                &["diff", "--name-only", "--diff-filter=U"],
                Some(&cwd),
                false,
                true,
            )
            .ok()
            .and_then(|r| {
                if r.returncode == 0 && !r.stdout.trim().is_empty() {
                    Some(r.stdout.trim().to_string())
                } else {
                    None
                }
            });

            let _ = git::git_command(&["rebase", "--abort"], Some(&cwd), false, false);

            let conflict_vec = conflicts
                .as_ref()
                .map(|c| c.lines().map(String::from).collect::<Vec<_>>());
            return Err(CwError::Rebase(messages::rebase_failed(
                &cwd.display().to_string(),
                &rebase_target,
                conflict_vec.as_deref(),
            )));
        }
    }

    println!("{} Rebase successful\n", style("*").green().bold());

    // Push
    if push {
        println!(
            "{}",
            style(format!("Pushing {} to origin...", feature_branch)).yellow()
        );
        match git::git_command(
            &["push", "-u", "origin", &feature_branch],
            Some(&cwd),
            false,
            true,
        ) {
            Ok(r) if r.returncode == 0 => {
                println!("{} Pushed to origin\n", style("*").green().bold());
            }
            Ok(r) => {
                // Try force push with lease
                match git::git_command(
                    &[
                        "push",
                        "--force-with-lease",
                        "-u",
                        "origin",
                        &feature_branch,
                    ],
                    Some(&cwd),
                    false,
                    true,
                ) {
                    Ok(r2) if r2.returncode == 0 => {
                        println!("{} Force pushed to origin\n", style("*").green().bold());
                    }
                    _ => {
                        return Err(CwError::Git(format!("Push failed: {}", r.stdout)));
                    }
                }
            }
            Err(e) => return Err(e),
        }
    }

    // Create PR
    println!("{}", style("Creating pull request...").yellow());

    let mut pr_args = vec![
        "gh".to_string(),
        "pr".to_string(),
        "create".to_string(),
        "--base".to_string(),
        base_branch.clone(),
    ];

    if let Some(t) = title {
        pr_args.extend(["--title".to_string(), t.to_string()]);
        if let Some(b) = body {
            pr_args.extend(["--body".to_string(), b.to_string()]);
        }
    } else {
        pr_args.push("--fill".to_string());
    }

    if draft {
        pr_args.push("--draft".to_string());
    }

    let output = Command::new(&pr_args[0])
        .args(&pr_args[1..])
        .current_dir(&cwd)
        .output()?;

    if output.status.success() {
        let pr_url = String::from_utf8_lossy(&output.stdout).trim().to_string();
        println!("{} Pull request created!\n", style("*").green().bold());
        println!("{} {}\n", style("PR URL:").bold(), pr_url);
        println!(
            "{}\n",
            style("Note: Worktree is still active. Use 'gw delete' to remove after PR is merged.")
                .dim()
        );

        // Post-PR hooks
        hook_ctx.insert("event".into(), "pr.post".into());
        hook_ctx.insert("pr_url".into(), pr_url);
        let _ = hooks::run_hooks("pr.post", &hook_ctx, Some(&cwd), Some(&base_path));
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CwError::Git(messages::pr_creation_failed(&stderr)));
    }

    Ok(())
}

/// Merge worktree: rebase, fast-forward merge, cleanup.
pub fn merge_worktree(
    target: Option<&str>,
    push: bool,
    _interactive: bool,
    dry_run: bool,
    ai_merge: bool,
    lookup_mode: Option<&str>,
) -> Result<()> {
    let (cwd, feature_branch, worktree_repo) = resolve_worktree_target(target, lookup_mode)?;
    let (base_branch, base_path) = get_worktree_metadata(&feature_branch, &worktree_repo)?;
    let repo = &base_path;

    println!("\n{}", style("Finishing worktree:").cyan().bold());
    println!("  Feature:     {}", style(&feature_branch).green());
    println!("  Base:        {}", style(&base_branch).green());
    println!("  Repo:        {}\n", style(repo.display()).blue());

    // Pre-merge hooks
    let mut hook_ctx = HashMap::new();
    hook_ctx.insert("branch".into(), feature_branch.clone());
    hook_ctx.insert("base_branch".into(), base_branch.clone());
    hook_ctx.insert("worktree_path".into(), cwd.to_string_lossy().to_string());
    hook_ctx.insert("repo_path".into(), repo.to_string_lossy().to_string());
    hook_ctx.insert("event".into(), "merge.pre".into());
    hook_ctx.insert("operation".into(), "merge".into());
    if !dry_run {
        hooks::run_hooks("merge.pre", &hook_ctx, Some(&cwd), Some(repo))?;
    }

    // Dry run
    if dry_run {
        println!(
            "{}\n",
            style("DRY RUN MODE — No changes will be made")
                .yellow()
                .bold()
        );
        println!(
            "{}\n",
            style("The following operations would be performed:").bold()
        );
        println!("  1. Fetch updates from remote");
        println!("  2. Rebase {} onto {}", feature_branch, base_branch);
        println!("  3. Switch to {} in base repository", base_branch);
        println!(
            "  4. Merge {} into {} (fast-forward)",
            feature_branch, base_branch
        );
        if push {
            println!("  5. Push {} to origin", base_branch);
            println!("  6. Remove worktree at {}", cwd.display());
            println!("  7. Delete local branch {}", feature_branch);
        } else {
            println!("  5. Remove worktree at {}", cwd.display());
            println!("  6. Delete local branch {}", feature_branch);
        }
        println!("\n{}\n", style("Run without --dry-run to execute.").dim());
        return Ok(());
    }

    // Fetch
    let fetch_ok = git::git_command(&["fetch", "--all", "--prune"], Some(repo), false, true)
        .map(|r| r.returncode == 0)
        .unwrap_or(false);

    let rebase_target = if fetch_ok {
        let origin_ref = format!("origin/{}", base_branch);
        if git::branch_exists(&origin_ref, Some(&cwd)) {
            origin_ref
        } else {
            base_branch.clone()
        }
    } else {
        base_branch.clone()
    };

    // Rebase
    println!(
        "{}",
        style(format!(
            "Rebasing {} onto {}...",
            feature_branch, rebase_target
        ))
        .yellow()
    );

    match git::git_command(&["rebase", &rebase_target], Some(&cwd), false, true) {
        Ok(r) if r.returncode == 0 => {}
        _ => {
            if ai_merge {
                // Try AI-assisted conflict resolution
                let conflicts = git::git_command(
                    &["diff", "--name-only", "--diff-filter=U"],
                    Some(&cwd),
                    false,
                    true,
                )
                .ok()
                .and_then(|r| {
                    if r.returncode == 0 && !r.stdout.trim().is_empty() {
                        Some(r.stdout.trim().to_string())
                    } else {
                        None
                    }
                });

                let _ = git::git_command(&["rebase", "--abort"], Some(&cwd), false, false);

                let conflict_list = conflicts.as_deref().unwrap_or("(unknown)");
                let prompt = format!(
                    "Resolve merge conflicts in this repository. The rebase of '{}' onto '{}' \
                     failed with conflicts in: {}\n\
                     Please examine the conflicted files and resolve them.",
                    feature_branch, rebase_target, conflict_list
                );

                println!(
                    "\n{} Launching AI to resolve conflicts...\n",
                    style("*").cyan().bold()
                );
                let _ = super::ai_tools::launch_ai_tool(&cwd, None, false, Some(&prompt));
                return Ok(());
            }

            let _ = git::git_command(&["rebase", "--abort"], Some(&cwd), false, false);
            return Err(CwError::Rebase(messages::rebase_failed(
                &cwd.display().to_string(),
                &rebase_target,
                None,
            )));
        }
    }

    println!("{} Rebase successful\n", style("*").green().bold());

    // Verify base path
    if !base_path.exists() {
        return Err(CwError::WorktreeNotFound(messages::base_repo_not_found(
            &base_path.display().to_string(),
        )));
    }

    // Fast-forward merge
    println!(
        "{}",
        style(format!(
            "Merging {} into {}...",
            feature_branch, base_branch
        ))
        .yellow()
    );

    // Switch to base branch if needed
    let _ = git::git_command(
        &["fetch", "--all", "--prune"],
        Some(&base_path),
        false,
        false,
    );
    if let Ok(current) = git::get_current_branch(Some(&base_path)) {
        if current != base_branch {
            git::git_command(&["switch", &base_branch], Some(&base_path), true, false)?;
        }
    } else {
        git::git_command(&["switch", &base_branch], Some(&base_path), true, false)?;
    }

    match git::git_command(
        &["merge", "--ff-only", &feature_branch],
        Some(&base_path),
        false,
        true,
    ) {
        Ok(r) if r.returncode == 0 => {}
        _ => {
            return Err(CwError::Merge(messages::merge_failed(
                &base_path.display().to_string(),
                &feature_branch,
            )));
        }
    }

    println!(
        "{} Merged {} into {}\n",
        style("*").green().bold(),
        feature_branch,
        base_branch
    );

    // Push
    if push {
        println!(
            "{}",
            style(format!("Pushing {} to origin...", base_branch)).yellow()
        );
        match git::git_command(
            &["push", "origin", &base_branch],
            Some(&base_path),
            false,
            true,
        ) {
            Ok(r) if r.returncode == 0 => {
                println!("{} Pushed to origin\n", style("*").green().bold());
            }
            _ => {
                println!("{} Push failed\n", style("!").yellow());
            }
        }
    }

    // Cleanup
    println!("{}", style("Cleaning up worktree and branch...").yellow());

    let _ = std::env::set_current_dir(repo);

    git::remove_worktree_safe(&cwd, repo, true)?;
    let _ = git::git_command(&["branch", "-D", &feature_branch], Some(repo), false, false);

    // Remove metadata
    let bb_key = format_config_key(CONFIG_KEY_BASE_BRANCH, &feature_branch);
    let bp_key = format_config_key(CONFIG_KEY_BASE_PATH, &feature_branch);
    let ib_key = format_config_key(CONFIG_KEY_INTENDED_BRANCH, &feature_branch);
    git::unset_config(&bb_key, Some(repo));
    git::unset_config(&bp_key, Some(repo));
    git::unset_config(&ib_key, Some(repo));

    println!("{}\n", style("* Cleanup complete!").green().bold());

    // Post-merge hooks
    hook_ctx.insert("event".into(), "merge.post".into());
    let _ = hooks::run_hooks("merge.post", &hook_ctx, Some(repo), Some(repo));
    let _ = registry::update_last_seen(repo);

    Ok(())
}
