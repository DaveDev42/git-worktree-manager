
use console::style;

use crate::constants::{format_config_key, CONFIG_KEY_BASE_BRANCH};
use crate::error::{CwError, Result};
use crate::git;

/// Change the base branch for a worktree.
pub fn change_base_branch(new_base: &str, branch: Option<&str>) -> Result<()> {
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

    git::set_config(&key, new_base, Some(&repo))?;

    println!(
        "\n{} Changed base branch for '{}'",
        style("*").green().bold(),
        style(&feature_branch).cyan(),
    );
    if let Some(old) = old_base {
        println!("  {} -> {}\n", style(&old).red(), style(new_base).green());
    } else {
        println!("  -> {}\n", style(new_base).green());
    }

    Ok(())
}
