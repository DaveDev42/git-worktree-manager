/// Integration tests for constants module.

#[test]
fn test_sanitize_branch_name_various() {
    use claude_worktree::constants::sanitize_branch_name;

    assert_eq!(sanitize_branch_name("feat/auth"), "feat-auth");
    assert_eq!(sanitize_branch_name("bugfix/issue-123"), "bugfix-issue-123");
    assert_eq!(sanitize_branch_name("feature/user@login"), "feature-user-login");
    assert_eq!(sanitize_branch_name("hotfix/v2.0"), "hotfix-v2.0");
    assert_eq!(sanitize_branch_name("///"), "worktree");
    assert_eq!(sanitize_branch_name(""), "worktree");
    assert_eq!(sanitize_branch_name("simple"), "simple");
    assert_eq!(sanitize_branch_name("a/b/c/d"), "a-b-c-d");
    assert_eq!(sanitize_branch_name("spaces here"), "spaces-here");
    assert_eq!(sanitize_branch_name("special!@#$%^chars"), "special-chars");
}

#[test]
fn test_default_worktree_path() {
    use claude_worktree::constants::default_worktree_path;
    use std::path::Path;

    let tmp = tempfile::TempDir::new().unwrap();
    let repo = tmp.path().join("myproject");
    std::fs::create_dir_all(&repo).unwrap();

    let result = default_worktree_path(&repo, "fix-auth");
    assert!(result
        .to_string_lossy()
        .ends_with("myproject-fix-auth"));

    let result = default_worktree_path(&repo, "feat/nested");
    assert!(result
        .to_string_lossy()
        .ends_with("myproject-feat-nested"));
}

#[test]
fn test_launch_method_all_variants() {
    use claude_worktree::constants::LaunchMethod;

    let methods = [
        LaunchMethod::Foreground,
        LaunchMethod::Detach,
        LaunchMethod::ItermWindow,
        LaunchMethod::ItermTab,
        LaunchMethod::ItermPaneH,
        LaunchMethod::ItermPaneV,
        LaunchMethod::Tmux,
        LaunchMethod::TmuxWindow,
        LaunchMethod::TmuxPaneH,
        LaunchMethod::TmuxPaneV,
        LaunchMethod::Zellij,
        LaunchMethod::ZellijTab,
        LaunchMethod::ZellijPaneH,
        LaunchMethod::ZellijPaneV,
        LaunchMethod::WeztermWindow,
        LaunchMethod::WeztermTab,
        LaunchMethod::WeztermPaneH,
        LaunchMethod::WeztermPaneV,
    ];

    // All 18 variants should roundtrip
    for m in &methods {
        let s = m.as_str();
        let parsed = LaunchMethod::from_str_opt(s);
        assert_eq!(parsed, Some(*m), "Roundtrip failed for {:?} -> {}", m, s);
    }
}

#[test]
fn test_launch_method_aliases_coverage() {
    use claude_worktree::constants::launch_method_aliases;

    let aliases = launch_method_aliases();
    // Verify key aliases exist
    assert_eq!(aliases["fg"], "foreground");
    assert_eq!(aliases["d"], "detach");
    assert_eq!(aliases["t"], "tmux");
    assert_eq!(aliases["z"], "zellij");
    assert_eq!(aliases["i-w"], "iterm-window");
    assert_eq!(aliases["w-t"], "wezterm-tab");
}

#[test]
fn test_format_config_key() {
    use claude_worktree::constants::*;
    assert_eq!(
        format_config_key(CONFIG_KEY_BASE_BRANCH, "fix-auth"),
        "branch.fix-auth.worktreeBase"
    );
    assert_eq!(
        format_config_key(CONFIG_KEY_BASE_PATH, "fix-auth"),
        "worktree.fix-auth.basePath"
    );
    assert_eq!(
        format_config_key(CONFIG_KEY_INTENDED_BRANCH, "fix-auth"),
        "worktree.fix-auth.intendedBranch"
    );
}
