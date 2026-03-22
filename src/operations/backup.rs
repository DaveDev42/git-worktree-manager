use std::path::{Path, PathBuf};

use console::style;
use serde::{Deserialize, Serialize};

use crate::config;
use crate::constants::{
    default_worktree_path, format_config_key, CONFIG_KEY_BASE_BRANCH, CONFIG_KEY_BASE_PATH,
};
use crate::error::{CwError, Result};
use crate::git;
use crate::messages;

#[derive(Debug, Serialize, Deserialize)]
struct BackupMetadata {
    branch: String,
    base_branch: Option<String>,
    base_path: Option<String>,
    worktree_path: String,
    backed_up_at: String,
    has_uncommitted_changes: bool,
    bundle_file: String,
    stash_file: Option<String>,
}

/// Get the backups directory.
fn get_backups_dir() -> PathBuf {
    let dir = config::get_config_path()
        .parent()
        .unwrap_or(Path::new("."))
        .join("backups");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Create backup of worktree(s) using git bundle.
pub fn backup_worktree(branch: Option<&str>, all: bool) -> Result<()> {
    let repo = git::get_repo_root(None)?;

    let branches_to_backup: Vec<(String, PathBuf)> = if all {
        git::get_feature_worktrees(Some(&repo))?
    } else {
        let resolved = super::helpers::resolve_worktree_target(branch, None)?;
        vec![(resolved.branch, resolved.path)]
    };

    let backups_root = get_backups_dir();
    let timestamp = crate::session::chrono_now_iso_pub()
        .replace([':', '-'], "")
        .split('T')
        .collect::<Vec<_>>()
        .join("-")
        .trim_end_matches('Z')
        .to_string();

    println!("\n{}\n", style("Creating backup(s)...").cyan().bold());

    let mut backup_count = 0;

    for (branch_name, worktree_path) in &branches_to_backup {
        let branch_backup_dir = backups_root.join(branch_name).join(&timestamp);
        let _ = std::fs::create_dir_all(&branch_backup_dir);

        let bundle_file = branch_backup_dir.join("bundle.git");
        let metadata_file = branch_backup_dir.join("metadata.json");

        println!(
            "{} {}",
            style("Backing up:").yellow(),
            style(branch_name).bold()
        );

        // Create git bundle
        let bundle_str = bundle_file.to_string_lossy().to_string();
        match git::git_command(
            &["bundle", "create", &bundle_str, "--all"],
            Some(worktree_path),
            false,
            true,
        ) {
            Ok(r) if r.returncode == 0 => {}
            _ => {
                println!("  {} Backup failed for {}", style("x").red(), branch_name);
                continue;
            }
        }

        // Get metadata
        let base_branch_key = format_config_key(CONFIG_KEY_BASE_BRANCH, branch_name);
        let base_path_key = format_config_key(CONFIG_KEY_BASE_PATH, branch_name);
        let base_branch = git::get_config(&base_branch_key, Some(&repo));
        let base_path = git::get_config(&base_path_key, Some(&repo));

        // Check for uncommitted changes
        let has_changes =
            git::git_command(&["status", "--porcelain"], Some(worktree_path), false, true)
                .map(|r| r.returncode == 0 && !r.stdout.trim().is_empty())
                .unwrap_or(false);

        // Create stash patch for uncommitted changes
        let stash_file = if has_changes {
            println!(
                "  {}",
                style("Found uncommitted changes, creating stash...").dim()
            );
            let patch_file = branch_backup_dir.join("stash.patch");
            if let Ok(r) = git::git_command(&["diff", "HEAD"], Some(worktree_path), false, true) {
                if let Err(e) = std::fs::write(&patch_file, &r.stdout) {
                    println!(
                        "  {} Failed to write stash patch: {}",
                        style("!").yellow(),
                        e
                    );
                }
                Some(patch_file.to_string_lossy().to_string())
            } else {
                None
            }
        } else {
            None
        };

        // Save metadata
        let metadata = BackupMetadata {
            branch: branch_name.clone(),
            base_branch,
            base_path,
            worktree_path: worktree_path.to_string_lossy().to_string(),
            backed_up_at: crate::session::chrono_now_iso_pub(),
            has_uncommitted_changes: has_changes,
            bundle_file: bundle_file.to_string_lossy().to_string(),
            stash_file,
        };

        match serde_json::to_string_pretty(&metadata) {
            Ok(content) => {
                if let Err(e) = std::fs::write(&metadata_file, content) {
                    println!(
                        "  {} Failed to write backup metadata: {}",
                        style("!").yellow(),
                        e
                    );
                }
            }
            Err(e) => {
                println!(
                    "  {} Failed to serialize backup metadata: {}",
                    style("!").yellow(),
                    e
                );
            }
        }

        println!(
            "  {} Backup saved to: {}",
            style("*").green(),
            branch_backup_dir.display()
        );
        backup_count += 1;
    }

    println!(
        "\n{}\n",
        style(format!(
            "* Backup complete! Created {} backup(s)",
            backup_count
        ))
        .green()
        .bold()
    );
    println!(
        "{}\n",
        style(format!("Backups saved in: {}", backups_root.display())).dim()
    );

    Ok(())
}

/// List available backups.
pub fn list_backups(branch: Option<&str>) -> Result<()> {
    let backups_dir = get_backups_dir();

    if !backups_dir.exists() {
        println!("\n{}\n", style("No backups found").yellow());
        return Ok(());
    }

    println!("\n{}\n", style("Available Backups:").cyan().bold());

    let mut found = false;

    let mut entries: Vec<_> = std::fs::read_dir(&backups_dir)?
        .flatten()
        .filter(|e| e.path().is_dir())
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for branch_dir in entries {
        let branch_name = branch_dir.file_name().to_string_lossy().to_string();

        if let Some(filter) = branch {
            if branch_name != filter {
                continue;
            }
        }

        let mut timestamps: Vec<_> = std::fs::read_dir(branch_dir.path())
            .ok()
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().is_dir())
            .collect();
        timestamps.sort_by_key(|e| std::cmp::Reverse(e.file_name()));

        if timestamps.is_empty() {
            continue;
        }

        found = true;
        println!("{}:", style(&branch_name).green().bold());

        for ts_dir in &timestamps {
            let metadata_file = ts_dir.path().join("metadata.json");
            if let Ok(content) = std::fs::read_to_string(&metadata_file) {
                if let Ok(meta) = serde_json::from_str::<BackupMetadata>(&content) {
                    let changes = if meta.has_uncommitted_changes {
                        format!(" {}", style("(with uncommitted changes)").yellow())
                    } else {
                        String::new()
                    };
                    println!(
                        "  - {} - {}{}",
                        ts_dir.file_name().to_string_lossy(),
                        meta.backed_up_at,
                        changes
                    );
                }
            }
        }
        println!();
    }

    if !found {
        println!("{}\n", style("No backups found").yellow());
    }

    Ok(())
}

/// Restore worktree from backup.
pub fn restore_worktree(branch: &str, path: Option<&str>, id: Option<&str>) -> Result<()> {
    let backups_dir = get_backups_dir();
    let branch_backup_dir = backups_dir.join(branch);

    if !branch_backup_dir.exists() {
        return Err(CwError::Git(messages::backup_not_found(
            id.unwrap_or("latest"),
            branch,
        )));
    }

    // Find backup by ID or use latest
    let backup_dir = if let Some(backup_id) = id {
        let specific_dir = branch_backup_dir.join(backup_id);
        if !specific_dir.exists() {
            return Err(CwError::Git(messages::backup_not_found(backup_id, branch)));
        }
        specific_dir
    } else {
        let mut backups: Vec<_> = std::fs::read_dir(&branch_backup_dir)?
            .flatten()
            .filter(|e| e.path().is_dir())
            .collect();
        backups.sort_by_key(|e| std::cmp::Reverse(e.file_name()));

        backups
            .first()
            .ok_or_else(|| CwError::Git(messages::backup_not_found("latest", branch)))?
            .path()
    };

    let backup_id = backup_dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let metadata_file = backup_dir.join("metadata.json");
    let bundle_file = backup_dir.join("bundle.git");

    if !metadata_file.exists() || !bundle_file.exists() {
        return Err(CwError::Git(
            "Invalid backup: missing metadata or bundle file".to_string(),
        ));
    }

    let content = std::fs::read_to_string(&metadata_file)?;
    let metadata: BackupMetadata = serde_json::from_str(&content)?;

    println!("\n{}", style("Restoring from backup:").cyan().bold());
    println!("  Branch: {}", style(branch).green());
    println!("  Backup ID: {}", style(&backup_id).yellow());
    println!("  Backed up at: {}\n", metadata.backed_up_at);

    let repo = git::get_repo_root(None)?;

    let worktree_path = if let Some(p) = path {
        PathBuf::from(p)
    } else {
        default_worktree_path(&repo, branch)
    };

    if worktree_path.exists() {
        return Err(CwError::Git(format!(
            "Worktree path already exists: {}\nRemove it first or specify --path",
            worktree_path.display()
        )));
    }

    // Clone from bundle
    if let Some(parent) = worktree_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    println!(
        "{} {}",
        style("Restoring worktree to:").yellow(),
        worktree_path.display()
    );

    let bundle_str = bundle_file.to_string_lossy().to_string();
    let wt_str = worktree_path.to_string_lossy().to_string();

    git::git_command(
        &["clone", &bundle_str, &wt_str],
        worktree_path.parent(),
        true,
        false,
    )?;

    let _ = git::git_command(&["checkout", branch], Some(&worktree_path), false, false);

    // Restore metadata
    if let Some(ref base_branch) = metadata.base_branch {
        let bb_key = format_config_key(CONFIG_KEY_BASE_BRANCH, branch);
        let bp_key = format_config_key(CONFIG_KEY_BASE_PATH, branch);
        let _ = git::set_config(&bb_key, base_branch, Some(&repo));
        let _ = git::set_config(&bp_key, &repo.to_string_lossy(), Some(&repo));
    }

    // Restore uncommitted changes
    let stash_file = backup_dir.join("stash.patch");
    if stash_file.exists() {
        println!("  {}", style("Restoring uncommitted changes...").dim());
        if let Ok(patch) = std::fs::read_to_string(&stash_file) {
            let mut child = std::process::Command::new("git")
                .args(["apply", "--whitespace=fix"])
                .current_dir(&worktree_path)
                .stdin(std::process::Stdio::piped())
                .spawn()
                .ok();

            if let Some(ref mut c) = child {
                if let Some(ref mut stdin) = c.stdin {
                    use std::io::Write;
                    let _ = stdin.write_all(patch.as_bytes());
                }
                let _ = c.wait();
            }
        }
    }

    println!("{} Restore complete!", style("*").green().bold());
    println!("  Worktree path: {}\n", worktree_path.display());

    Ok(())
}
