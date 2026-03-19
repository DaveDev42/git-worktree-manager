/// Tests for constants module.
/// Ported from tests/test_constants.py (12 tests → 16 tests).
use git_worktree_manager::constants::*;

#[test]
fn test_sanitize_branch_name_simple() {
    assert_eq!(sanitize_branch_name("fix-auth"), "fix-auth");
    assert_eq!(sanitize_branch_name("feature"), "feature");
    assert_eq!(sanitize_branch_name("v1.0"), "v1.0");
}

#[test]
fn test_sanitize_branch_name_with_slashes() {
    assert_eq!(sanitize_branch_name("feat/auth"), "feat-auth");
    assert_eq!(sanitize_branch_name("bugfix/issue-123"), "bugfix-issue-123");
    assert_eq!(
        sanitize_branch_name("feature/user/login"),
        "feature-user-login"
    );
    assert_eq!(sanitize_branch_name("hotfix/v2.0"), "hotfix-v2.0");
}

#[test]
fn test_sanitize_branch_name_special_characters() {
    assert_eq!(sanitize_branch_name("feature<test>"), "feature-test");
    assert_eq!(sanitize_branch_name("fix:issue"), "fix-issue");
    assert_eq!(sanitize_branch_name(r#"bug"quote"#), "bug-quote");
    assert_eq!(sanitize_branch_name("test|pipe"), "test-pipe");
    assert_eq!(sanitize_branch_name("star*test"), "star-test");
    assert_eq!(sanitize_branch_name("back\\slash"), "back-slash");
    assert_eq!(sanitize_branch_name("question?mark"), "question-mark");
}

#[test]
fn test_sanitize_branch_name_whitespace() {
    assert_eq!(sanitize_branch_name("fix auth"), "fix-auth");
    assert_eq!(
        sanitize_branch_name("feature  multi  space"),
        "feature-multi-space"
    );
    assert_eq!(sanitize_branch_name("tab\there"), "tab-here");
}

#[test]
fn test_sanitize_branch_name_multiple_hyphens() {
    assert_eq!(sanitize_branch_name("feat//auth"), "feat-auth");
    assert_eq!(sanitize_branch_name("fix///bug"), "fix-bug");
    assert_eq!(sanitize_branch_name("test<<>>name"), "test-name");
}

#[test]
fn test_sanitize_branch_name_leading_trailing() {
    assert_eq!(sanitize_branch_name("/feat/auth"), "feat-auth");
    assert_eq!(sanitize_branch_name("feat/auth/"), "feat-auth");
    assert_eq!(sanitize_branch_name("/feat/auth/"), "feat-auth");
    assert_eq!(sanitize_branch_name("-branch-"), "branch");
}

#[test]
fn test_sanitize_branch_name_edge_cases() {
    assert_eq!(sanitize_branch_name("///"), "worktree");
    assert_eq!(sanitize_branch_name("***"), "worktree");
    assert_eq!(sanitize_branch_name("   "), "worktree");
    assert_eq!(sanitize_branch_name("a"), "a");
    assert_eq!(sanitize_branch_name("/"), "worktree");
    assert_eq!(sanitize_branch_name(""), "worktree");
}

#[test]
fn test_sanitize_branch_name_unicode() {
    assert_eq!(sanitize_branch_name("feature-日本語"), "feature-日本語");
    assert_eq!(sanitize_branch_name("émoji-test"), "émoji-test");
}

#[test]
fn test_sanitize_branch_name_complex() {
    assert_eq!(
        sanitize_branch_name("feature/USER-123/add-authentication"),
        "feature-USER-123-add-authentication"
    );
    assert_eq!(sanitize_branch_name("bugfix/issue#456"), "bugfix-issue-456");
    assert_eq!(
        sanitize_branch_name("release/v2.0.1-beta"),
        "release-v2.0.1-beta"
    );
}

#[test]
fn test_default_worktree_path_simple() {
    let tmp = tempfile::TempDir::new().unwrap();
    let repo = tmp.path().join("myproject");
    std::fs::create_dir_all(&repo).unwrap();

    let result = default_worktree_path(&repo, "fix-auth");
    assert_eq!(
        result.file_name().unwrap().to_str().unwrap(),
        "myproject-fix-auth"
    );
    let result = default_worktree_path(&repo, "feature");
    assert_eq!(
        result.file_name().unwrap().to_str().unwrap(),
        "myproject-feature"
    );
}

#[test]
fn test_default_worktree_path_with_slashes() {
    let tmp = tempfile::TempDir::new().unwrap();
    let repo = tmp.path().join("myproject");
    std::fs::create_dir_all(&repo).unwrap();

    let result = default_worktree_path(&repo, "feat/auth");
    assert_eq!(
        result.file_name().unwrap().to_str().unwrap(),
        "myproject-feat-auth"
    );
    let result = default_worktree_path(&repo, "release/v2.0");
    assert_eq!(
        result.file_name().unwrap().to_str().unwrap(),
        "myproject-release-v2.0"
    );
}

#[test]
fn test_default_worktree_path_special_chars() {
    let tmp = tempfile::TempDir::new().unwrap();
    let repo = tmp.path().join("myproject");
    std::fs::create_dir_all(&repo).unwrap();

    let result = default_worktree_path(&repo, "fix:auth");
    assert_eq!(
        result.file_name().unwrap().to_str().unwrap(),
        "myproject-fix-auth"
    );
    let result = default_worktree_path(&repo, "feature<test>");
    assert_eq!(
        result.file_name().unwrap().to_str().unwrap(),
        "myproject-feature-test"
    );
}

#[test]
fn test_launch_method_all_18_variants_roundtrip() {
    let all = [
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
    assert_eq!(all.len(), 18);
    for m in &all {
        assert_eq!(LaunchMethod::from_str_opt(m.as_str()), Some(*m));
    }
}

#[test]
fn test_launch_method_aliases_complete() {
    let aliases = launch_method_aliases();
    assert_eq!(aliases.len(), 18);
    assert_eq!(aliases["fg"], "foreground");
    assert_eq!(aliases["d"], "detach");
    assert_eq!(aliases["t"], "tmux");
    assert_eq!(aliases["z"], "zellij");
    assert_eq!(aliases["i-w"], "iterm-window");
    assert_eq!(aliases["w-t"], "wezterm-tab");
}

#[test]
fn test_format_config_keys() {
    assert_eq!(
        format_config_key(CONFIG_KEY_BASE_BRANCH, "fix"),
        "branch.fix.worktreeBase"
    );
    assert_eq!(
        format_config_key(CONFIG_KEY_BASE_PATH, "fix"),
        "worktree.fix.basePath"
    );
    assert_eq!(
        format_config_key(CONFIG_KEY_INTENDED_BRANCH, "fix"),
        "worktree.fix.intendedBranch"
    );
}

#[test]
fn test_max_session_name_length() {
    assert_eq!(MAX_SESSION_NAME_LENGTH, 50);
    assert_eq!(CLAUDE_SESSION_PREFIX_LENGTH, 200);
}
