/// Git operations wrapper utilities.
///
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use crate::constants::sanitize_branch_name;
use crate::error::{CwError, Result};

/// Canonicalize a path, falling back to the original path on failure.
pub fn canonicalize_or(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// Result of running a command, with stdout captured as String.
#[derive(Debug)]
pub struct CommandResult {
    pub stdout: String,
    pub returncode: i32,
}

/// Run a shell command.
pub fn run_command(
    cmd: &[&str],
    cwd: Option<&Path>,
    check: bool,
    capture: bool,
) -> Result<CommandResult> {
    if cmd.is_empty() {
        return Err(CwError::Git("Empty command".to_string()));
    }

    let mut command = Command::new(cmd[0]);
    command.args(&cmd[1..]);

    if let Some(dir) = cwd {
        command.current_dir(dir);
    }

    if capture {
        command.stdout(std::process::Stdio::piped());
        command.stderr(std::process::Stdio::piped());
    }

    let output: Output = command.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            CwError::Git(format!("Command not found: {}", cmd[0]))
        } else {
            CwError::Io(e)
        }
    })?;

    let returncode = output.status.code().unwrap_or(-1);
    let stdout = if capture {
        // Merge stderr into stdout like Python's STDOUT redirect
        let mut out = String::from_utf8_lossy(&output.stdout).to_string();
        let err = String::from_utf8_lossy(&output.stderr);
        if !err.is_empty() {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&err);
        }
        out
    } else {
        String::new()
    };

    if check && returncode != 0 {
        return Err(CwError::Git(format!(
            "Command failed: {}\n{}",
            cmd.join(" "),
            stdout
        )));
    }

    Ok(CommandResult { stdout, returncode })
}

/// Run a git command.
pub fn git_command(
    args: &[&str],
    repo: Option<&Path>,
    check: bool,
    capture: bool,
) -> Result<CommandResult> {
    let mut cmd = vec!["git"];
    cmd.extend_from_slice(args);
    run_command(&cmd, repo, check, capture)
}

/// Get the root directory of the git repository.
pub fn get_repo_root(path: Option<&Path>) -> Result<PathBuf> {
    let result = git_command(&["rev-parse", "--show-toplevel"], path, true, true);
    match result {
        Ok(r) => Ok(PathBuf::from(r.stdout.trim())),
        Err(_) => Err(CwError::Git("Not in a git repository".to_string())),
    }
}

/// Get the current branch name.
pub fn get_current_branch(repo: Option<&Path>) -> Result<String> {
    let result = git_command(&["rev-parse", "--abbrev-ref", "HEAD"], repo, true, true)?;
    let branch = result.stdout.trim().to_string();
    if branch == "HEAD" {
        return Err(CwError::InvalidBranch("In detached HEAD state".to_string()));
    }
    Ok(branch)
}

/// Auto-detect the repository's default branch.
///
/// Priority:
/// 1. `origin/HEAD` symref (most reliable — set by `git clone`)
/// 2. Local `main` branch exists
/// 3. Local `master` branch exists
/// 4. Config fallback
pub fn detect_default_branch(repo: Option<&Path>) -> String {
    // 1. origin/HEAD
    if let Ok(r) = git_command(
        &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
        repo,
        false,
        true,
    ) {
        if r.returncode == 0 {
            let branch = r
                .stdout
                .trim()
                .strip_prefix("origin/")
                .unwrap_or(r.stdout.trim());
            if !branch.is_empty() {
                return branch.to_string();
            }
        }
    }

    // 2. main
    if branch_exists("main", repo) {
        return "main".to_string();
    }

    // 3. master
    if branch_exists("master", repo) {
        return "master".to_string();
    }

    // 4. config fallback
    crate::config::load_config()
        .map(|c| c.git.default_base_branch)
        .unwrap_or_else(|_| "main".to_string())
}

/// Check if a branch exists.
pub fn branch_exists(branch: &str, repo: Option<&Path>) -> bool {
    git_command(&["rev-parse", "--verify", branch], repo, false, true)
        .map(|r| r.returncode == 0)
        .unwrap_or(false)
}

/// Check if a branch exists on a remote.
pub fn remote_branch_exists(branch: &str, repo: Option<&Path>, remote: &str) -> bool {
    let ref_name = format!("{}/{}", remote, branch);
    git_command(&["rev-parse", "--verify", &ref_name], repo, false, true)
        .map(|r| r.returncode == 0)
        .unwrap_or(false)
}

/// Get a git config value (local scope).
pub fn get_config(key: &str, repo: Option<&Path>) -> Option<String> {
    git_command(&["config", "--local", "--get", key], repo, false, true)
        .ok()
        .and_then(|r| {
            if r.returncode == 0 {
                Some(r.stdout.trim().to_string())
            } else {
                None
            }
        })
}

/// Set a git config value (local scope).
pub fn set_config(key: &str, value: &str, repo: Option<&Path>) -> Result<()> {
    git_command(&["config", "--local", key, value], repo, true, false)?;
    Ok(())
}

/// Unset a git config value.
pub fn unset_config(key: &str, repo: Option<&Path>) {
    let _ = git_command(
        &["config", "--local", "--unset-all", key],
        repo,
        false,
        false,
    );
}

/// Normalize branch name by removing refs/heads/ prefix if present.
pub fn normalize_branch_name(branch: &str) -> &str {
    branch.strip_prefix("refs/heads/").unwrap_or(branch)
}

/// Parsed worktree entry: (branch_or_detached, path).
pub type WorktreeEntry = (String, PathBuf);

/// Parse `git worktree list --porcelain` output.
pub fn parse_worktrees(repo: &Path) -> Result<Vec<WorktreeEntry>> {
    let result = git_command(&["worktree", "list", "--porcelain"], Some(repo), true, true)?;

    let mut items: Vec<WorktreeEntry> = Vec::new();
    let mut cur_path: Option<String> = None;
    let mut cur_branch: Option<String> = None;

    for line in result.stdout.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            cur_path = Some(path.to_string());
        } else if let Some(branch) = line.strip_prefix("branch ") {
            cur_branch = Some(branch.to_string());
        } else if line.trim().is_empty() {
            if let Some(path) = cur_path.take() {
                let branch = cur_branch
                    .take()
                    .unwrap_or_else(|| "(detached)".to_string());
                items.push((branch, PathBuf::from(path)));
            }
        }
    }
    // Handle last entry (no trailing blank line)
    if let Some(path) = cur_path {
        let branch = cur_branch.unwrap_or_else(|| "(detached)".to_string());
        items.push((branch, PathBuf::from(path)));
    }

    Ok(items)
}

/// Get feature worktrees, excluding main repo and detached entries.
pub fn get_feature_worktrees(repo: Option<&Path>) -> Result<Vec<(String, PathBuf)>> {
    let effective_repo = get_repo_root(repo)?;
    let worktrees = parse_worktrees(&effective_repo)?;
    if worktrees.is_empty() {
        return Ok(Vec::new());
    }

    let main_path = canonicalize_or(&worktrees[0].1);

    let mut result = Vec::new();
    for (branch, path) in &worktrees {
        let resolved = canonicalize_or(path);
        if resolved == main_path {
            continue;
        }
        if branch == "(detached)" {
            continue;
        }
        let branch_name = normalize_branch_name(branch).to_string();
        result.push((branch_name, path.clone()));
    }
    Ok(result)
}

/// Get main repository path, even when called from a worktree.
pub fn get_main_repo_root(repo: Option<&Path>) -> Result<PathBuf> {
    let current_root = get_repo_root(repo)?;
    let worktrees = parse_worktrees(&current_root)?;
    if let Some(first) = worktrees.first() {
        Ok(first.1.clone())
    } else {
        Ok(current_root)
    }
}

/// Find worktree path by branch name.
pub fn find_worktree_by_branch(repo: &Path, branch: &str) -> Result<Option<PathBuf>> {
    let worktrees = parse_worktrees(repo)?;
    Ok(worktrees
        .into_iter()
        .find(|(br, _)| br == branch)
        .map(|(_, path)| path))
}

/// Find worktree by directory name.
pub fn find_worktree_by_name(repo: &Path, worktree_name: &str) -> Result<Option<PathBuf>> {
    let worktrees = parse_worktrees(repo)?;
    Ok(worktrees
        .into_iter()
        .find(|(_, path)| {
            path.file_name()
                .map(|n| n.to_string_lossy() == worktree_name)
                .unwrap_or(false)
        })
        .map(|(_, path)| path))
}

/// Find worktree path by intended branch name (from metadata).
pub fn find_worktree_by_intended_branch(
    repo: &Path,
    intended_branch: &str,
) -> Result<Option<PathBuf>> {
    let intended_branch = normalize_branch_name(intended_branch);

    // Strategy 1: Direct lookup by current branch name
    if let Some(path) = find_worktree_by_branch(repo, intended_branch)? {
        return Ok(Some(path));
    }
    // Also try with refs/heads/ prefix
    let with_prefix = format!("refs/heads/{}", intended_branch);
    if let Some(path) = find_worktree_by_branch(repo, &with_prefix)? {
        return Ok(Some(path));
    }

    // Strategy 2: Search all intended branch metadata
    let result = git_command(
        &[
            "config",
            "--local",
            "--get-regexp",
            r"^worktree\..*\.intendedBranch",
        ],
        Some(repo),
        false,
        true,
    )?;

    if result.returncode == 0 {
        for line in result.stdout.trim().lines() {
            let parts: Vec<&str> = line.splitn(2, char::is_whitespace).collect();
            if parts.len() == 2 {
                let key = parts[0];
                let value = parts[1];
                // Extract branch name from key: worktree.<branch>.intendedBranch
                let key_parts: Vec<&str> = key.split('.').collect();
                if key_parts.len() >= 2 {
                    let branch_from_key = key_parts[1];
                    if branch_from_key == intended_branch || value == intended_branch {
                        let worktrees = parse_worktrees(repo)?;
                        let repo_name = repo
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();
                        let expected_suffix =
                            format!("{}-{}", repo_name, sanitize_branch_name(branch_from_key));
                        for (_, path) in &worktrees {
                            if let Some(name) = path.file_name() {
                                if name.to_string_lossy() == expected_suffix {
                                    return Ok(Some(path.clone()));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Strategy 3: Fallback — check path naming convention
    let repo_name = repo
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let expected_suffix = format!("{}-{}", repo_name, sanitize_branch_name(intended_branch));
    let worktrees = parse_worktrees(repo)?;
    let repo_resolved = canonicalize_or(repo);

    for (_, path) in &worktrees {
        if let Some(name) = path.file_name() {
            if name.to_string_lossy() == expected_suffix {
                let path_resolved = canonicalize_or(path);
                if path_resolved != repo_resolved {
                    return Ok(Some(path.clone()));
                }
            }
        }
    }

    Ok(None)
}

/// Fetch from remote and determine the rebase target for a base branch.
///
/// Returns `(fetch_ok, rebase_target)` where `rebase_target` is `origin/<base>`
/// if fetch succeeded and the remote ref exists, otherwise just `<base>`.
pub fn fetch_and_rebase_target(base_branch: &str, repo: &Path, cwd: &Path) -> (bool, String) {
    let fetch_ok = git_command(&["fetch", "--all", "--prune"], Some(repo), false, true)
        .map(|r| r.returncode == 0)
        .unwrap_or(false);

    let rebase_target = if fetch_ok {
        let origin_ref = format!("origin/{}", base_branch);
        if branch_exists(&origin_ref, Some(cwd)) {
            origin_ref
        } else {
            base_branch.to_string()
        }
    } else {
        base_branch.to_string()
    };

    (fetch_ok, rebase_target)
}

/// Check if a command is available in PATH.
pub fn has_command(name: &str) -> bool {
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return true;
            }
            // On Windows, try with .exe extension
            #[cfg(target_os = "windows")]
            {
                let with_ext = dir.join(format!("{}.exe", name));
                if with_ext.is_file() {
                    return true;
                }
            }
        }
    }
    false
}

/// Check if running in non-interactive environment.
pub fn is_non_interactive() -> bool {
    // Explicit flag
    if let Ok(val) = std::env::var("CW_NON_INTERACTIVE") {
        let val = val.to_lowercase();
        if val == "1" || val == "true" || val == "yes" {
            return true;
        }
    }

    // Check stdin is not a TTY
    if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        return true;
    }

    // CI environment variables
    let ci_vars = [
        "CI",
        "GITHUB_ACTIONS",
        "GITLAB_CI",
        "JENKINS_HOME",
        "CIRCLECI",
        "TRAVIS",
        "BUILDKITE",
        "DRONE",
        "BITBUCKET_PIPELINE",
        "CODEBUILD_BUILD_ID",
    ];

    ci_vars.iter().any(|var| std::env::var(var).is_ok())
}

/// Check if a branch name is valid according to git rules.
pub fn is_valid_branch_name(branch_name: &str, repo: Option<&Path>) -> bool {
    if branch_name.is_empty() {
        return false;
    }
    git_command(
        &["check-ref-format", "--branch", branch_name],
        repo,
        false,
        true,
    )
    .map(|r| r.returncode == 0)
    .unwrap_or(false)
}

/// Get descriptive error message for invalid branch name.
pub fn get_branch_name_error(branch_name: &str) -> String {
    if branch_name.is_empty() {
        return "Branch name cannot be empty".to_string();
    }
    if branch_name == "@" {
        return "Branch name cannot be '@' alone".to_string();
    }
    if branch_name.ends_with(".lock") {
        return "Branch name cannot end with '.lock'".to_string();
    }
    if branch_name.starts_with('/') || branch_name.ends_with('/') {
        return "Branch name cannot start or end with '/'".to_string();
    }
    if branch_name.contains("//") {
        return "Branch name cannot contain consecutive slashes '//'".to_string();
    }
    if branch_name.contains("..") {
        return "Branch name cannot contain consecutive dots '..'".to_string();
    }
    if branch_name.contains("@{") {
        return "Branch name cannot contain '@{'".to_string();
    }

    let invalid_chars: &[char] = &['~', '^', ':', '?', '*', '[', '\\'];
    let found: Vec<char> = invalid_chars
        .iter()
        .filter(|&&c| branch_name.contains(c))
        .copied()
        .collect();
    if !found.is_empty() {
        let chars_display: Vec<String> = found.iter().map(|c| format!("{:?}", c)).collect();
        return format!(
            "Branch name contains invalid characters: {}",
            chars_display.join(", ")
        );
    }

    if branch_name.chars().any(|c| (c as u32) < 32 || c == ' ') {
        return "Branch name cannot contain spaces or control characters".to_string();
    }

    format!(
        "'{}' is not a valid branch name. See 'git check-ref-format --help' for rules",
        branch_name
    )
}

/// Remove a git worktree with platform-safe fallback.
pub fn remove_worktree_safe(worktree_path: &Path, repo: &Path, force: bool) -> Result<()> {
    let worktree_str = canonicalize_or(worktree_path).to_string_lossy().to_string();

    let mut args = vec!["worktree", "remove", &worktree_str];
    if force {
        args.push("--force");
    }

    let result = git_command(&args, Some(repo), false, true)?;

    if result.returncode == 0 {
        return Ok(());
    }

    // Windows fallback for "Directory not empty"
    #[cfg(target_os = "windows")]
    {
        if result.stdout.contains("Directory not empty") {
            let path = PathBuf::from(&worktree_str);
            if path.exists() {
                std::fs::remove_dir_all(&path).map_err(|e| {
                    CwError::Git(format!(
                        "Failed to remove worktree directory on Windows: {}\nError: {}",
                        worktree_str, e
                    ))
                })?;
            }
            git_command(&["worktree", "prune"], Some(repo), true, false)?;
            return Ok(());
        }
    }

    Err(CwError::Git(format!(
        "Command failed: {}\n{}",
        args.join(" "),
        result.stdout
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(not(windows))]
    fn test_canonicalize_or_existing_path() {
        // /tmp should exist on all Unix systems
        let path = Path::new("/tmp");
        let result = canonicalize_or(path);
        // Should resolve to a real path (e.g., /private/tmp on macOS)
        assert!(result.is_absolute());
    }

    #[test]
    fn test_canonicalize_or_nonexistent_path() {
        let path = Path::new("/nonexistent/path/that/does/not/exist");
        let result = canonicalize_or(path);
        // Should return the original path as-is
        assert_eq!(result, path);
    }

    #[test]
    fn test_canonicalize_or_relative_path() {
        let path = Path::new("relative/path");
        let result = canonicalize_or(path);
        // Non-existent relative path should return as-is
        assert_eq!(result, path);
    }

    #[test]
    fn test_normalize_branch_name() {
        assert_eq!(normalize_branch_name("refs/heads/main"), "main");
        assert_eq!(normalize_branch_name("feature-branch"), "feature-branch");
        assert_eq!(normalize_branch_name("refs/heads/feat/auth"), "feat/auth");
    }

    #[test]
    fn test_get_branch_name_error() {
        assert_eq!(get_branch_name_error(""), "Branch name cannot be empty");
        assert_eq!(
            get_branch_name_error("@"),
            "Branch name cannot be '@' alone"
        );
        assert_eq!(
            get_branch_name_error("foo.lock"),
            "Branch name cannot end with '.lock'"
        );
        assert_eq!(
            get_branch_name_error("/foo"),
            "Branch name cannot start or end with '/'"
        );
        assert_eq!(
            get_branch_name_error("foo//bar"),
            "Branch name cannot contain consecutive slashes '//'"
        );
    }
}
