//! Standardized error and informational messages for git-worktree-manager.
//!

pub fn worktree_not_found(branch: &str) -> String {
    format!(
        "No worktree found for branch '{}'. Use 'gw list' to see available worktrees.",
        branch
    )
}

pub fn target_not_found(target: &str) -> String {
    format!(
        "No worktree matches '{}' (tried name, branch, path). Use 'gw list' to see available worktrees.",
        target
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

pub fn worktree_dir_not_found(path: &str) -> String {
    format!("Worktree directory does not exist: {}", path)
}

pub fn cannot_delete_main_worktree() -> String {
    "Cannot delete main repository worktree".to_string()
}

pub fn detached_head_warning() -> String {
    "Worktree is detached or branch not found. Specify branch with --branch or skip with --force."
        .to_string()
}

// ---------------------------------------------------------------------------
// Status / progress messages (used in styled println! calls)
// ---------------------------------------------------------------------------

pub fn deleting_local_branch(branch: &str) -> String {
    format!("Deleting local branch: {}", branch)
}

pub fn deleting_remote_branch(branch: &str) -> String {
    format!("Deleting remote branch: origin/{}", branch)
}

pub fn removing_worktree(path: &std::path::Path) -> String {
    format!("Removing worktree: {}", path.display())
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

pub fn worktree_unlocking_stale(path: &str, pid: u32) -> String {
    format!("info: stale lock from dead pid {pid} on {path} — unlocking")
}

pub fn worktree_locked_live_pid(path: &str, pid: u32) -> String {
    format!(
        "Worktree {path} is locked by pid {pid} which is still running — \
         refusing to unlock. Stop that process or unlock manually with \
         `git worktree unlock {path}`."
    )
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
    fn test_target_not_found() {
        let msg = target_not_found("my-target");
        assert!(msg.contains("my-target"));
        assert!(msg.contains("name, branch, path"));
        assert!(msg.contains("gw list"));
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
    fn test_worktree_dir_not_found() {
        let msg = worktree_dir_not_found("/tmp/worktree");
        assert!(msg.contains("/tmp/worktree"));
        assert!(msg.contains("does not exist"));
    }

    #[test]
    fn test_cannot_delete_main_worktree() {
        let msg = cannot_delete_main_worktree();
        assert!(msg.contains("Cannot delete main repository worktree"));
    }

    #[test]
    fn test_detached_head_warning() {
        let msg = detached_head_warning();
        assert!(msg.contains("detached"));
        assert!(msg.contains("--branch"));
        assert!(msg.contains("--force"));
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
