/// Configuration-related operations.
///
/// Mirrors src/claude_worktree/operations/config_ops.py.
use std::path::PathBuf;

use console::style;
use serde_json::Value;

use crate::config;
use crate::constants::{format_config_key, CONFIG_KEY_BASE_BRANCH, CONFIG_KEY_BASE_PATH};
use crate::error::{CwError, Result};
use crate::git;

/// Change the base branch for a worktree (with optional rebase + dry-run).
pub fn change_base_branch(new_base: &str, branch: Option<&str>, dry_run: bool) -> Result<()> {
    let repo = git::get_repo_root(None)?;

    let feature_branch = if let Some(b) = branch {
        b.to_string()
    } else {
        git::get_current_branch(Some(&std::env::current_dir()?))?
    };

    // Verify new base exists
    if !git::branch_exists(new_base, Some(&repo)) {
        return Err(CwError::InvalidBranch(format!(
            "Base branch '{}' not found",
            new_base
        )));
    }

    let key = format_config_key(CONFIG_KEY_BASE_BRANCH, &feature_branch);
    let old_base = git::get_config(&key, Some(&repo));

    println!("\n{}", style("Changing base branch:").cyan().bold());
    println!("  Worktree:    {}", style(&feature_branch).green());
    if let Some(ref old) = old_base {
        println!("  Current base: {}", style(old).yellow());
    }
    println!("  New base:     {}\n", style(new_base).green());

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
        println!("  2. Rebase {} onto {}", feature_branch, new_base);
        println!(
            "  3. Update base branch metadata: {} -> {}",
            old_base.as_deref().unwrap_or("none"),
            new_base
        );
        println!("\n{}\n", style("Run without --dry-run to execute.").dim());
        return Ok(());
    }

    // Fetch
    let _ = git::git_command(&["fetch", "--all", "--prune"], Some(&repo), false, false);

    // Rebase onto new base
    let rebase_target = {
        let origin = format!("origin/{}", new_base);
        if git::branch_exists(&origin, Some(&repo)) {
            origin
        } else {
            new_base.to_string()
        }
    };

    println!(
        "{}",
        style(format!(
            "Rebasing {} onto {}...",
            feature_branch, rebase_target
        ))
        .yellow()
    );

    // Find worktree path for rebase
    let wt_path = git::find_worktree_by_branch(&repo, &feature_branch)?
        .or(git::find_worktree_by_branch(
            &repo,
            &format!("refs/heads/{}", feature_branch),
        )?)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    match git::git_command(&["rebase", &rebase_target], Some(&wt_path), false, true) {
        Ok(r) if r.returncode == 0 => {
            println!("{} Rebase successful\n", style("*").green().bold());
        }
        _ => {
            let _ = git::git_command(&["rebase", "--abort"], Some(&wt_path), false, false);
            return Err(CwError::Rebase(format!(
                "Rebase failed. Resolve conflicts manually:\n  cd {}\n  git rebase {}",
                wt_path.display(),
                rebase_target
            )));
        }
    }

    // Update metadata
    git::set_config(&key, new_base, Some(&repo))?;
    println!(
        "{} Base branch changed to '{}'\n",
        style("*").green().bold(),
        new_base
    );

    Ok(())
}

/// Export worktree configuration to a file.
pub fn export_config(output: Option<&str>) -> Result<()> {
    let repo = git::get_repo_root(None)?;
    let cfg = config::load_config()?;

    let mut worktrees_data: Vec<Value> = Vec::new();

    for (branch_name, path) in git::get_feature_worktrees(Some(&repo))? {
        let bb_key = format_config_key(CONFIG_KEY_BASE_BRANCH, &branch_name);
        let bp_key = format_config_key(CONFIG_KEY_BASE_PATH, &branch_name);
        let base_branch = git::get_config(&bb_key, Some(&repo));
        let base_path = git::get_config(&bp_key, Some(&repo));

        worktrees_data.push(serde_json::json!({
            "branch": branch_name,
            "base_branch": base_branch,
            "base_path": base_path,
            "path": path.to_string_lossy(),
        }));
    }

    let export_data = serde_json::json!({
        "export_version": "1.0",
        "exported_at": crate::session::chrono_now_iso_pub(),
        "repository": repo.to_string_lossy(),
        "config": serde_json::to_value(&cfg)?,
        "worktrees": worktrees_data,
    });

    let timestamp = crate::session::chrono_now_iso_pub()
        .replace([':', '-'], "")
        .split('T')
        .collect::<Vec<_>>()
        .join("-")
        .trim_end_matches('Z')
        .to_string();

    let output_path = output
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("cw-export-{}.json", timestamp));

    println!(
        "\n{} {}",
        style("Exporting configuration to:").yellow(),
        output_path
    );

    let content = serde_json::to_string_pretty(&export_data)?;
    std::fs::write(&output_path, content)?;

    println!("{} Export complete!\n", style("*").green().bold());
    println!("{}", style("Exported:").bold());
    println!("  - {} worktree(s)", worktrees_data.len());
    println!("  - Configuration settings");
    println!(
        "\n{}\n",
        style("Transfer this file and use 'cw import' to restore.").dim()
    );

    Ok(())
}

/// Import worktree configuration from a file.
pub fn import_config(import_file: &str, apply: bool) -> Result<()> {
    let path = PathBuf::from(import_file);
    if !path.exists() {
        return Err(CwError::Config(format!(
            "Import file not found: {}",
            import_file
        )));
    }

    println!(
        "\n{} {}\n",
        style("Loading import file:").yellow(),
        import_file
    );

    let content = std::fs::read_to_string(&path)?;
    let data: Value = serde_json::from_str(&content)
        .map_err(|e| CwError::Config(format!("Failed to read import file: {}", e)))?;

    if data.get("export_version").is_none() {
        return Err(CwError::Config("Invalid export file format".to_string()));
    }

    // Preview
    println!("{}\n", style("Import Preview:").cyan().bold());
    println!(
        "{} {}",
        style("Exported from:").bold(),
        data.get("repository")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
    );
    println!(
        "{} {}",
        style("Exported at:").bold(),
        data.get("exported_at")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
    );
    let worktrees = data
        .get("worktrees")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    println!("{} {}\n", style("Worktrees:").bold(), worktrees.len());

    for wt in &worktrees {
        println!(
            "  - {}",
            wt.get("branch")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
        );
        println!(
            "    Base: {}",
            wt.get("base_branch")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
        );
    }

    if !apply {
        println!(
            "\n{} No changes made. Use --apply to import configuration.\n",
            style("Preview mode:").yellow().bold()
        );
        return Ok(());
    }

    // Apply
    println!("\n{}\n", style("Applying import...").yellow().bold());

    let repo = git::get_repo_root(None)?;
    let mut imported = 0u32;

    // Import global config
    if let Some(cfg_val) = data.get("config") {
        if let Ok(cfg) = serde_json::from_value::<config::Config>(cfg_val.clone()) {
            println!("{}", style("Importing global configuration...").yellow());
            config::save_config(&cfg)?;
            println!("{} Configuration imported\n", style("*").green().bold());
        }
    }

    // Import worktree metadata
    println!("{}\n", style("Importing worktree metadata...").yellow());
    for wt in &worktrees {
        let branch = wt.get("branch").and_then(|v| v.as_str());
        let base = wt.get("base_branch").and_then(|v| v.as_str());

        if let (Some(b), Some(bb)) = (branch, base) {
            if !git::branch_exists(b, Some(&repo)) {
                println!(
                    "{} Branch '{}' not found locally. Create with 'cw new {} --base {}'",
                    style("!").yellow(),
                    b,
                    b,
                    bb
                );
                continue;
            }

            let bb_key = format_config_key(CONFIG_KEY_BASE_BRANCH, b);
            let bp_key = format_config_key(CONFIG_KEY_BASE_PATH, b);
            let _ = git::set_config(&bb_key, bb, Some(&repo));
            let _ = git::set_config(&bp_key, &repo.to_string_lossy(), Some(&repo));
            println!("{} Imported metadata for: {}", style("*").green().bold(), b);
            imported += 1;
        }
    }

    println!(
        "\n{}\n",
        style(format!(
            "* Import complete! Imported {} worktree(s)",
            imported
        ))
        .green()
        .bold()
    );

    Ok(())
}
