/// Constants and default values for git-worktree-manager.
///
/// Mirrors src/git_worktree_manager/constants.py.
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use regex::Regex;
use serde::{Deserialize, Serialize};

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
    // Characters unsafe for directory names across platforms
    let unsafe_re = Regex::new(r#"[/<>:"|?*\\#@&;$`!~%^()\[\]{}=+]+"#).unwrap();
    let whitespace_re = Regex::new(r"\s+").unwrap();
    let multi_hyphen_re = Regex::new(r"-+").unwrap();

    let safe = unsafe_re.replace_all(branch_name, "-");
    let safe = whitespace_re.replace_all(&safe, "-");
    let safe = multi_hyphen_re.replace_all(&safe, "-");
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
}
