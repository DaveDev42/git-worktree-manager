/// Constants and default values for git-worktree-manager.
///
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

/// Pre-compiled regex patterns for branch name sanitization.
static UNSAFE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"[/<>:"|?*\\#@&;$`!~%^()\[\]{}=+]+"#).unwrap());
static WHITESPACE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());
static MULTI_HYPHEN_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"-+").unwrap());

/// Terminal launch methods for AI tool execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LaunchMethod {
    Foreground,
    Detach,
    // iTerm (macOS)
    ItermWindow,
    ItermTab,
    ItermPaneH,
    ItermPaneV,
    // tmux
    Tmux,
    TmuxWindow,
    TmuxPaneH,
    TmuxPaneV,
    // Zellij
    Zellij,
    ZellijTab,
    ZellijPaneH,
    ZellijPaneV,
    // WezTerm
    WeztermWindow,
    WeztermTab,
    WeztermPaneH,
    WeztermPaneV,
}

impl LaunchMethod {
    /// Convert to the canonical kebab-case string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Foreground => "foreground",
            Self::Detach => "detach",
            Self::ItermWindow => "iterm-window",
            Self::ItermTab => "iterm-tab",
            Self::ItermPaneH => "iterm-pane-h",
            Self::ItermPaneV => "iterm-pane-v",
            Self::Tmux => "tmux",
            Self::TmuxWindow => "tmux-window",
            Self::TmuxPaneH => "tmux-pane-h",
            Self::TmuxPaneV => "tmux-pane-v",
            Self::Zellij => "zellij",
            Self::ZellijTab => "zellij-tab",
            Self::ZellijPaneH => "zellij-pane-h",
            Self::ZellijPaneV => "zellij-pane-v",
            Self::WeztermWindow => "wezterm-window",
            Self::WeztermTab => "wezterm-tab",
            Self::WeztermPaneH => "wezterm-pane-h",
            Self::WeztermPaneV => "wezterm-pane-v",
        }
    }

    /// Parse from a kebab-case string.
    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s {
            "foreground" => Some(Self::Foreground),
            "detach" => Some(Self::Detach),
            "iterm-window" => Some(Self::ItermWindow),
            "iterm-tab" => Some(Self::ItermTab),
            "iterm-pane-h" => Some(Self::ItermPaneH),
            "iterm-pane-v" => Some(Self::ItermPaneV),
            "tmux" => Some(Self::Tmux),
            "tmux-window" => Some(Self::TmuxWindow),
            "tmux-pane-h" => Some(Self::TmuxPaneH),
            "tmux-pane-v" => Some(Self::TmuxPaneV),
            "zellij" => Some(Self::Zellij),
            "zellij-tab" => Some(Self::ZellijTab),
            "zellij-pane-h" => Some(Self::ZellijPaneH),
            "zellij-pane-v" => Some(Self::ZellijPaneV),
            "wezterm-window" => Some(Self::WeztermWindow),
            "wezterm-tab" => Some(Self::WeztermTab),
            "wezterm-pane-h" => Some(Self::WeztermPaneH),
            "wezterm-pane-v" => Some(Self::WeztermPaneV),
            _ => None,
        }
    }
}

impl std::fmt::Display for LaunchMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Build the alias map for launch methods.
/// First letter: i=iTerm, t=tmux, z=Zellij, w=WezTerm
/// Second: w=window, t=tab, p=pane
/// For panes: h=horizontal, v=vertical
pub fn launch_method_aliases() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("fg", "foreground"),
        ("d", "detach"),
        // iTerm
        ("i-w", "iterm-window"),
        ("i-t", "iterm-tab"),
        ("i-p-h", "iterm-pane-h"),
        ("i-p-v", "iterm-pane-v"),
        // tmux
        ("t", "tmux"),
        ("t-w", "tmux-window"),
        ("t-p-h", "tmux-pane-h"),
        ("t-p-v", "tmux-pane-v"),
        // Zellij
        ("z", "zellij"),
        ("z-t", "zellij-tab"),
        ("z-p-h", "zellij-pane-h"),
        ("z-p-v", "zellij-pane-v"),
        // WezTerm
        ("w-w", "wezterm-window"),
        ("w-t", "wezterm-tab"),
        ("w-p-h", "wezterm-pane-h"),
        ("w-p-v", "wezterm-pane-v"),
    ])
}

/// Seconds in one day (24 * 60 * 60).
pub const SECS_PER_DAY: u64 = 86400;

/// Seconds in one day as f64 (for floating-point age calculations).
pub const SECS_PER_DAY_F64: f64 = 86400.0;

/// Minimum required Git version for worktree features.
pub const MIN_GIT_VERSION: &str = "2.31.0";

/// Minimum Git major version.
pub const MIN_GIT_VERSION_MAJOR: u32 = 2;

/// Minimum Git minor version (when major == MIN_GIT_VERSION_MAJOR).
pub const MIN_GIT_VERSION_MINOR: u32 = 31;

/// Timeout in seconds for AI tool execution (e.g., PR description generation).
pub const AI_TOOL_TIMEOUT_SECS: u64 = 60;

/// Poll interval in milliseconds when waiting for AI tool completion.
pub const AI_TOOL_POLL_MS: u64 = 100;

/// Maximum session name length for tmux/zellij compatibility.
/// Zellij uses Unix sockets which have a ~108 byte path limit.
pub const MAX_SESSION_NAME_LENGTH: usize = 50;

/// Claude native session path prefix length threshold.
pub const CLAUDE_SESSION_PREFIX_LENGTH: usize = 200;

/// Git config key templates for metadata storage.
pub const CONFIG_KEY_BASE_BRANCH: &str = "branch.{}.worktreeBase";
pub const CONFIG_KEY_BASE_PATH: &str = "worktree.{}.basePath";
pub const CONFIG_KEY_INTENDED_BRANCH: &str = "worktree.{}.intendedBranch";

/// Format a git config key by replacing `{}` with the branch name.
pub fn format_config_key(template: &str, branch: &str) -> String {
    template.replace("{}", branch)
}

/// Return the user's home directory, falling back to `"."` if unavailable.
pub fn home_dir_or_fallback() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

/// Compute the age of a file in fractional days, or `None` on error.
pub fn path_age_days(path: &Path) -> Option<f64> {
    let mtime = path.metadata().and_then(|m| m.modified()).ok()?;
    std::time::SystemTime::now()
        .duration_since(mtime)
        .ok()
        .map(|d| d.as_secs_f64() / SECS_PER_DAY_F64)
}

/// Check if a semver version string meets a minimum (major, minor).
pub fn version_meets_minimum(version_str: &str, min_major: u32, min_minor: u32) -> bool {
    let parts: Vec<u32> = version_str
        .split('.')
        .filter_map(|p| p.parse().ok())
        .collect();
    parts.len() >= 2 && (parts[0] > min_major || (parts[0] == min_major && parts[1] >= min_minor))
}

/// Convert branch name to safe directory name.
///
/// Handles branch names with slashes (feat/auth), special characters,
/// and other filesystem-unsafe characters.
///
/// # Examples
/// ```
/// use git_worktree_manager::constants::sanitize_branch_name;
/// assert_eq!(sanitize_branch_name("feat/auth"), "feat-auth");
/// assert_eq!(sanitize_branch_name("feature/user@login"), "feature-user-login");
/// assert_eq!(sanitize_branch_name("hotfix/v2.0"), "hotfix-v2.0");
/// ```
pub fn sanitize_branch_name(branch_name: &str) -> String {
    let safe = UNSAFE_RE.replace_all(branch_name, "-");
    let safe = WHITESPACE_RE.replace_all(&safe, "-");
    let safe = MULTI_HYPHEN_RE.replace_all(&safe, "-");
    let safe = safe.trim_matches('-');

    if safe.is_empty() {
        "worktree".to_string()
    } else {
        safe.to_string()
    }
}

/// Generate default worktree path: `../<repo>-<branch>`.
pub fn default_worktree_path(repo_path: &Path, branch_name: &str) -> PathBuf {
    let repo_path = strip_unc(
        repo_path
            .canonicalize()
            .unwrap_or_else(|_| repo_path.to_path_buf()),
    );
    let safe_branch = sanitize_branch_name(branch_name);
    let repo_name = repo_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "repo".to_string());

    repo_path
        .parent()
        .unwrap_or(repo_path.as_path())
        .join(format!("{}-{}", repo_name, safe_branch))
}

/// Strip Windows UNC path prefix (`\\?\`) which `canonicalize()` adds.
/// Git doesn't understand UNC paths, so we must strip them.
pub fn strip_unc(path: PathBuf) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        let s = path.to_string_lossy();
        if let Some(stripped) = s.strip_prefix(r"\\?\") {
            return PathBuf::from(stripped);
        }
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_branch_name() {
        assert_eq!(sanitize_branch_name("feat/auth"), "feat-auth");
        assert_eq!(sanitize_branch_name("bugfix/issue-123"), "bugfix-issue-123");
        assert_eq!(
            sanitize_branch_name("feature/user@login"),
            "feature-user-login"
        );
        assert_eq!(sanitize_branch_name("hotfix/v2.0"), "hotfix-v2.0");
        assert_eq!(sanitize_branch_name("///"), "worktree");
        assert_eq!(sanitize_branch_name(""), "worktree");
        assert_eq!(sanitize_branch_name("simple"), "simple");
    }

    #[test]
    fn test_launch_method_roundtrip() {
        for method in [
            LaunchMethod::Foreground,
            LaunchMethod::Detach,
            LaunchMethod::ItermWindow,
            LaunchMethod::Tmux,
            LaunchMethod::Zellij,
            LaunchMethod::WeztermTab,
        ] {
            let s = method.as_str();
            assert_eq!(LaunchMethod::from_str_opt(s), Some(method));
        }
    }

    #[test]
    fn test_format_config_key() {
        assert_eq!(
            format_config_key(CONFIG_KEY_BASE_BRANCH, "fix-auth"),
            "branch.fix-auth.worktreeBase"
        );
    }

    #[test]
    fn test_home_dir_or_fallback() {
        let home = home_dir_or_fallback();
        // Should return a non-empty path (either real home or ".")
        assert!(!home.as_os_str().is_empty());
    }

    #[test]
    fn test_path_age_days() {
        // Non-existent path returns None
        assert!(path_age_days(std::path::Path::new("/nonexistent/path")).is_none());

        // Existing path returns Some with non-negative value
        let tmp = std::env::temp_dir();
        if let Some(age) = path_age_days(&tmp) {
            assert!(age >= 0.0);
        }
    }

    #[test]
    fn test_version_meets_minimum() {
        assert!(version_meets_minimum("2.31.0", 2, 31));
        assert!(version_meets_minimum("2.40.0", 2, 31));
        assert!(version_meets_minimum("3.0.0", 2, 31));
        assert!(!version_meets_minimum("2.30.0", 2, 31));
        assert!(!version_meets_minimum("1.99.0", 2, 31));
        assert!(!version_meets_minimum("", 2, 31));
        assert!(!version_meets_minimum("2", 2, 31));
    }
}
