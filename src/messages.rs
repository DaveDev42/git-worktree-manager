//! Standardized error and informational messages for git-worktree-manager.
//!
//! Mirrors src/git_worktree_manager/messages.py.

pub fn worktree_not_found(branch: &str) -> String {
    format!(
        "No worktree found for branch '{}'. Use 'gw list' to see available worktrees.",
        branch
    )
}

pub fn branch_not_found(branch: &str) -> String {
    format!("Branch '{}' not found", branch)
}

pub fn invalid_branch_name(error_msg: &str) -> String {
    format!(
        "Invalid branch name: {}\nHint: Use alphanumeric characters, hyphens, and slashes. \
         Avoid special characters like emojis, backslashes, or control characters.",
        error_msg
    )
}

pub fn cannot_determine_branch() -> String {
    "Cannot determine current branch".to_string()
}

pub fn cannot_determine_base_branch() -> String {
    "Cannot determine base branch. Specify with --base or checkout a branch first.".to_string()
}

pub fn missing_metadata(branch: &str) -> String {
    format!(
        "Missing metadata for branch '{}'. Was this worktree created with 'gw new'?",
        branch
    )
}

pub fn base_repo_not_found(path: &str) -> String {
    format!("Base repository not found at: {}", path)
}

pub fn worktree_dir_not_found(path: &str) -> String {
    format!("Worktree directory does not exist: {}", path)
}

pub fn rebase_failed(
    worktree_path: &str,
    rebase_target: &str,
    conflicted_files: Option<&[String]>,
) -> String {
    let mut msg = format!(
        "Rebase failed. Please resolve conflicts manually:\n  cd {}\n  git rebase {}",
        worktree_path, rebase_target
    );
    if let Some(files) = conflicted_files {
        msg.push_str(&format!("\n\nConflicted files ({}):", files.len()));
        for file in files {
            msg.push_str(&format!("\n  \u{2022} {}", file));
        }
        msg.push_str("\n\nTip: Use --ai-merge flag to get AI assistance with conflicts");
    }
    msg
}

pub fn merge_failed(base_path: &str, feature_branch: &str) -> String {
    format!(
        "Fast-forward merge failed. Manual intervention required:\n  cd {}\n  git merge {}",
        base_path, feature_branch
    )
}

pub fn pr_creation_failed(stderr: &str) -> String {
    format!("Failed to create pull request: {}", stderr)
}

pub fn gh_cli_not_found() -> String {
    "GitHub CLI (gh) is required to create pull requests.\n\
     Install it from: https://cli.github.com/"
        .to_string()
}

pub fn cannot_delete_main_worktree() -> String {
    "Cannot delete main repository worktree".to_string()
}

pub fn stash_not_found(stash_ref: &str) -> String {
    format!(
        "Stash '{}' not found. Use 'gw stash list' to see available stashes.",
        stash_ref
    )
}

pub fn backup_not_found(backup_id: &str, branch: &str) -> String {
    format!("Backup '{}' not found for branch '{}'", backup_id, branch)
}

pub fn import_file_not_found(import_file: &str) -> String {
    format!("Import file not found: {}", import_file)
}

pub fn detached_head_warning() -> String {
    "Worktree is detached or branch not found. Specify branch with --branch or skip with --force."
        .to_string()
}

// ---------------------------------------------------------------------------
// Status / progress messages (used in styled println! calls)
// ---------------------------------------------------------------------------

pub fn rebase_in_progress(branch: &str, target: &str) -> String {
    format!("Rebasing {} onto {}...", branch, target)
}

pub fn pushing_to_origin(branch: &str) -> String {
    format!("Pushing {} to origin...", branch)
}

pub fn deleting_local_branch(branch: &str) -> String {
    format!("Deleting local branch: {}", branch)
}

pub fn deleting_remote_branch(branch: &str) -> String {
    format!("Deleting remote branch: origin/{}", branch)
}

pub fn removing_worktree(path: &std::path::Path) -> String {
    format!("Removing worktree: {}", path.display())
}

pub fn cleanup_complete(deleted: u32) -> String {
    format!("* Cleanup complete! Deleted {} worktree(s)", deleted)
}

pub fn starting_ai_tool_foreground(tool_name: &str) -> String {
    format!("Starting {} (Ctrl+C to exit)...", tool_name)
}

pub fn starting_ai_tool_in(tool_name: &str) -> String {
    format!("Starting {} in:", tool_name)
}

pub fn resuming_ai_tool_in(tool_name: &str) -> String {
    format!("Resuming {} in:", tool_name)
}

pub fn switched_to_worktree(path: &std::path::Path) -> String {
    format!("Switched to worktree: {}", path.display())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worktree_not_found() {
        let msg = worktree_not_found("feature-x");
        assert!(msg.contains("feature-x"));
        assert!(msg.contains("gw list"));
        assert!(msg.contains("No worktree found"));
    }

    #[test]
    fn test_branch_not_found() {
        let msg = branch_not_found("my-branch");
        assert!(msg.contains("my-branch"));
        assert!(msg.contains("not found"));
    }

    #[test]
    fn test_invalid_branch_name() {
        let msg = invalid_branch_name("contains spaces");
        assert!(msg.contains("contains spaces"));
        assert!(msg.contains("Invalid branch name"));
        assert!(msg.contains("Hint"));
        assert!(msg.contains("alphanumeric"));
    }

    #[test]
    fn test_cannot_determine_branch() {
        let msg = cannot_determine_branch();
        assert!(msg.contains("Cannot determine current branch"));
    }

    #[test]
    fn test_cannot_determine_base_branch() {
        let msg = cannot_determine_base_branch();
        assert!(msg.contains("Cannot determine base branch"));
        assert!(msg.contains("--base"));
    }

    #[test]
    fn test_missing_metadata() {
        let msg = missing_metadata("feat-login");
        assert!(msg.contains("feat-login"));
        assert!(msg.contains("Missing metadata"));
        assert!(msg.contains("gw new"));
    }

    #[test]
    fn test_base_repo_not_found() {
        let msg = base_repo_not_found("/tmp/repo");
        assert!(msg.contains("/tmp/repo"));
        assert!(msg.contains("Base repository not found"));
    }

    #[test]
    fn test_worktree_dir_not_found() {
        let msg = worktree_dir_not_found("/tmp/worktree");
        assert!(msg.contains("/tmp/worktree"));
        assert!(msg.contains("does not exist"));
    }

    #[test]
    fn test_rebase_failed_without_conflicts() {
        let msg = rebase_failed("/tmp/wt", "main", None);
        assert!(msg.contains("Rebase failed"));
        assert!(msg.contains("cd /tmp/wt"));
        assert!(msg.contains("git rebase main"));
        assert!(!msg.contains("Conflicted files"));
    }

    #[test]
    fn test_rebase_failed_with_conflicts() {
        let files = vec!["src/main.rs".to_string(), "Cargo.toml".to_string()];
        let msg = rebase_failed("/tmp/wt", "main", Some(&files));
        assert!(msg.contains("Rebase failed"));
        assert!(msg.contains("cd /tmp/wt"));
        assert!(msg.contains("git rebase main"));
        assert!(msg.contains("Conflicted files (2)"));
        assert!(msg.contains("src/main.rs"));
        assert!(msg.contains("Cargo.toml"));
        assert!(msg.contains("--ai-merge"));
    }

    #[test]
    fn test_rebase_failed_with_empty_conflicts() {
        let files: Vec<String> = vec![];
        let msg = rebase_failed("/tmp/wt", "main", Some(&files));
        assert!(msg.contains("Conflicted files (0)"));
    }

    #[test]
    fn test_merge_failed() {
        let msg = merge_failed("/tmp/base", "feature-api");
        assert!(msg.contains("Fast-forward merge failed"));
        assert!(msg.contains("cd /tmp/base"));
        assert!(msg.contains("git merge feature-api"));
    }

    #[test]
    fn test_pr_creation_failed() {
        let msg = pr_creation_failed("permission denied");
        assert!(msg.contains("Failed to create pull request"));
        assert!(msg.contains("permission denied"));
    }

    #[test]
    fn test_gh_cli_not_found() {
        let msg = gh_cli_not_found();
        assert!(msg.contains("GitHub CLI (gh)"));
        assert!(msg.contains("https://cli.github.com/"));
    }

    #[test]
    fn test_cannot_delete_main_worktree() {
        let msg = cannot_delete_main_worktree();
        assert!(msg.contains("Cannot delete main repository worktree"));
    }

    #[test]
    fn test_stash_not_found() {
        let msg = stash_not_found("stash@{0}");
        assert!(msg.contains("stash@{0}"));
        assert!(msg.contains("gw stash list"));
    }

    #[test]
    fn test_backup_not_found() {
        let msg = backup_not_found("abc123", "feature-x");
        assert!(msg.contains("abc123"));
        assert!(msg.contains("feature-x"));
        assert!(msg.contains("not found"));
    }

    #[test]
    fn test_import_file_not_found() {
        let msg = import_file_not_found("/tmp/export.json");
        assert!(msg.contains("/tmp/export.json"));
        assert!(msg.contains("Import file not found"));
    }

    #[test]
    fn test_detached_head_warning() {
        let msg = detached_head_warning();
        assert!(msg.contains("detached"));
        assert!(msg.contains("--branch"));
        assert!(msg.contains("--force"));
    }

    #[test]
    fn test_rebase_in_progress() {
        let msg = rebase_in_progress("feat-x", "main");
        assert!(msg.contains("Rebasing feat-x onto main"));
    }

    #[test]
    fn test_pushing_to_origin() {
        let msg = pushing_to_origin("feat-x");
        assert!(msg.contains("Pushing feat-x to origin"));
    }

    #[test]
    fn test_deleting_local_branch() {
        let msg = deleting_local_branch("feat-x");
        assert!(msg.contains("Deleting local branch: feat-x"));
    }

    #[test]
    fn test_deleting_remote_branch() {
        let msg = deleting_remote_branch("feat-x");
        assert!(msg.contains("origin/feat-x"));
    }

    #[test]
    fn test_removing_worktree() {
        let msg = removing_worktree(std::path::Path::new("/tmp/wt"));
        assert!(msg.contains("Removing worktree:"));
        assert!(msg.contains("/tmp/wt"));
    }

    #[test]
    fn test_cleanup_complete() {
        let msg = cleanup_complete(3);
        assert!(msg.contains("3 worktree(s)"));
    }

    #[test]
    fn test_starting_ai_tool_foreground() {
        let msg = starting_ai_tool_foreground("claude");
        assert!(msg.contains("Starting claude"));
        assert!(msg.contains("Ctrl+C"));
    }

    #[test]
    fn test_starting_ai_tool_in() {
        let msg = starting_ai_tool_in("claude");
        assert_eq!(msg, "Starting claude in:");
    }

    #[test]
    fn test_resuming_ai_tool_in() {
        let msg = resuming_ai_tool_in("claude");
        assert_eq!(msg, "Resuming claude in:");
    }

    #[test]
    fn test_switched_to_worktree() {
        let msg = switched_to_worktree(std::path::Path::new("/tmp/wt"));
        assert!(msg.contains("Switched to worktree:"));
        assert!(msg.contains("/tmp/wt"));
    }
}
