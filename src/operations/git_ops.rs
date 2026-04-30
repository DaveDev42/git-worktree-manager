/// Git operations for merging.
///
use std::process::Command;

use console::style;

use crate::constants::{
    format_config_key, CONFIG_KEY_BASE_BRANCH, CONFIG_KEY_BASE_PATH, CONFIG_KEY_INTENDED_BRANCH,
};
use crate::error::{CwError, Result};
use crate::git;
use crate::hooks;
use crate::registry;

use super::helpers::{build_hook_context, get_worktree_metadata, resolve_worktree_target};
use crate::messages;

/// Merge worktree: rebase, fast-forward merge, cleanup.
pub fn merge_worktree(
    target: Option<&str>,
    push: bool,
    interactive: bool,
    dry_run: bool,
    ai_merge: bool,
    lookup_mode: Option<&str>,
) -> Result<()> {
    let resolved = resolve_worktree_target(target, lookup_mode)?;
    let cwd = resolved.path;
    let feature_branch = resolved.branch;
    let (base_branch, base_path) = get_worktree_metadata(&feature_branch, &resolved.repo)?;
    let repo = &base_path;

    println!("\n{}", style("Finishing worktree:").cyan().bold());
    println!("  Feature:     {}", style(&feature_branch).green());
    println!("  Base:        {}", style(&base_branch).green());
    println!("  Repo:        {}\n", style(repo.display()).blue());

    // Pre-merge hooks
    let mut hook_ctx = build_hook_context(
        &feature_branch,
        &base_branch,
        &cwd,
        repo,
        "merge.pre",
        "merge",
    );
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

    // Fetch and determine rebase target
    let (_fetch_ok, rebase_target) = git::fetch_and_rebase_target(&base_branch, repo, &cwd);

    // Rebase
    if interactive {
        // Interactive rebase requires a TTY — run directly via inherited stdio
        println!(
            "{}",
            style(format!(
                "Interactive rebase of {} onto {}...",
                feature_branch, rebase_target
            ))
            .yellow()
        );
        let status = Command::new("git")
            .args(["rebase", "-i", &rebase_target])
            .current_dir(&cwd)
            .status();
        match status {
            Ok(s) if s.success() => {}
            _ => {
                return Err(CwError::Rebase(messages::rebase_failed(
                    &cwd.display().to_string(),
                    &rebase_target,
                    None,
                )));
            }
        }
    } else {
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
                    let conflicts = git::list_conflicted_files(&cwd);
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
                    let _ = super::ai_tools::launch_ai_tool(
                        &cwd,
                        None,
                        false,
                        Some(&prompt),
                        None,
                        false,
                        false,
                    );
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
            style(messages::pushing_to_origin(&base_branch)).yellow()
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
