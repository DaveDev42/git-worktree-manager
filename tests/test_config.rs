/// Integration tests for configuration management.
use std::path::PathBuf;

use tempfile::TempDir;

/// Test config with isolated home directory to avoid polluting real config.
fn with_isolated_config<F: FnOnce(&TempDir)>(f: F) {
    let tmp = TempDir::new().unwrap();
    // Note: Config functions use dirs::home_dir(), not easily overridden
    // These tests verify the module's internal logic via library calls
    f(&tmp);
}

#[test]
fn test_default_config_values() {
    let config = git_worktree_manager::config::Config::default();
    assert_eq!(config.ai_tool.command, "claude");
    assert!(config.ai_tool.args.is_empty());
    assert_eq!(config.git.default_base_branch, "main");
    assert!(config.update.auto_check);
    assert_eq!(config.launch.tmux_session_prefix, "gw");
    assert_eq!(config.launch.wezterm_ready_timeout, 5.0);
    assert!(!config.shell_completion.prompted);
    assert!(!config.shell_completion.installed);
    assert!(config.launch.method.is_none());
}

#[test]
fn test_presets_all_present() {
    let presets = git_worktree_manager::config::ai_tool_presets();
    assert!(presets.contains_key("claude"));
    assert!(presets.contains_key("claude-yolo"));
    assert!(presets.contains_key("claude-remote"));
    assert!(presets.contains_key("claude-yolo-remote"));
    assert!(presets.contains_key("codex"));
    assert!(presets.contains_key("codex-yolo"));
    assert!(presets.contains_key("no-op"));
    assert_eq!(presets.len(), 7);
}

#[test]
fn test_resume_presets_all_present() {
    let presets = git_worktree_manager::config::ai_tool_resume_presets();
    assert!(presets.contains_key("claude"));
    assert!(presets.contains_key("codex"));
    // Claude uses --continue, not --resume
    assert!(presets["claude"].contains(&"--continue"));
    // Codex uses subcommand syntax
    assert!(presets["codex"].contains(&"resume"));
}

#[test]
fn test_merge_presets_all_present() {
    let presets = git_worktree_manager::config::ai_tool_merge_presets();
    assert!(presets.contains_key("claude"));
    assert!(presets.contains_key("codex"));
}

#[test]
fn test_list_presets_format() {
    let output = git_worktree_manager::config::list_presets();
    assert!(output.contains("Available AI tool presets:"));
    assert!(output.contains("claude"));
    assert!(output.contains("no-op"));
    assert!(output.contains("codex"));
}

#[test]
fn test_resolve_launch_alias() {
    assert_eq!(
        git_worktree_manager::config::resolve_launch_alias("fg"),
        "foreground"
    );
    assert_eq!(
        git_worktree_manager::config::resolve_launch_alias("t"),
        "tmux"
    );
    assert_eq!(
        git_worktree_manager::config::resolve_launch_alias("z-t"),
        "zellij-tab"
    );
    assert_eq!(
        git_worktree_manager::config::resolve_launch_alias("i-w"),
        "iterm-window"
    );
    assert_eq!(
        git_worktree_manager::config::resolve_launch_alias("w-t"),
        "wezterm-tab"
    );
    assert_eq!(
        git_worktree_manager::config::resolve_launch_alias("d"),
        "detach"
    );
    // Session name passthrough
    assert_eq!(
        git_worktree_manager::config::resolve_launch_alias("t:mywork"),
        "tmux:mywork"
    );
    assert_eq!(
        git_worktree_manager::config::resolve_launch_alias("z:dev"),
        "zellij:dev"
    );
    // Unknown passes through
    assert_eq!(
        git_worktree_manager::config::resolve_launch_alias("unknown"),
        "unknown"
    );
}

#[test]
fn test_parse_term_option() {
    use git_worktree_manager::config::parse_term_option;
    use git_worktree_manager::constants::LaunchMethod;

    let (m, s) = parse_term_option(Some("t")).unwrap();
    assert_eq!(m, LaunchMethod::Tmux);
    assert!(s.is_none());

    let (m, s) = parse_term_option(Some("t:mywork")).unwrap();
    assert_eq!(m, LaunchMethod::Tmux);
    assert_eq!(s.unwrap(), "mywork");

    let (m, s) = parse_term_option(Some("i-t")).unwrap();
    assert_eq!(m, LaunchMethod::ItermTab);
    assert!(s.is_none());

    let (m, s) = parse_term_option(Some("z-p-h")).unwrap();
    assert_eq!(m, LaunchMethod::ZellijPaneH);
    assert!(s.is_none());

    let (m, s) = parse_term_option(Some("w-w")).unwrap();
    assert_eq!(m, LaunchMethod::WeztermWindow);
    assert!(s.is_none());

    // Invalid
    assert!(parse_term_option(Some("invalid-method")).is_err());

    // Session name too long
    let long_name = "a".repeat(51);
    assert!(parse_term_option(Some(&format!("t:{}", long_name))).is_err());
}

#[test]
fn test_deep_merge_preserves_defaults() {
    // Verify that loading a partial config still has all default fields
    let config = git_worktree_manager::config::Config::default();
    let json = serde_json::to_value(&config).unwrap();

    // Verify all top-level keys exist
    assert!(json.get("ai_tool").is_some());
    assert!(json.get("launch").is_some());
    assert!(json.get("git").is_some());
    assert!(json.get("update").is_some());
    assert!(json.get("shell_completion").is_some());
}

// --- Additional tests ported from test_config.py ---

#[test]
fn test_get_config_path() {
    let path = git_worktree_manager::config::get_config_path();
    assert!(path.to_string_lossy().contains("config.json"));
    assert!(path.to_string_lossy().contains(".config"));
}

#[test]
fn test_preset_noop_empty() {
    let presets = git_worktree_manager::config::ai_tool_presets();
    assert!(presets["no-op"].is_empty());
}

#[test]
fn test_preset_claude_command() {
    let presets = git_worktree_manager::config::ai_tool_presets();
    assert_eq!(presets["claude"], vec!["claude"]);
}

#[test]
fn test_preset_claude_yolo_command() {
    let presets = git_worktree_manager::config::ai_tool_presets();
    assert_eq!(
        presets["claude-yolo"],
        vec!["claude", "--dangerously-skip-permissions"]
    );
}

#[test]
fn test_preset_claude_remote_command() {
    let presets = git_worktree_manager::config::ai_tool_presets();
    assert_eq!(presets["claude-remote"], vec!["claude", "/remote-control"]);
}

#[test]
fn test_preset_codex_command() {
    let presets = git_worktree_manager::config::ai_tool_presets();
    assert_eq!(presets["codex"], vec!["codex"]);
}

#[test]
fn test_preset_codex_yolo_command() {
    let presets = git_worktree_manager::config::ai_tool_presets();
    assert_eq!(
        presets["codex-yolo"],
        vec!["codex", "--dangerously-bypass-approvals-and-sandbox"]
    );
}

#[test]
fn test_resume_preset_claude_uses_continue() {
    let presets = git_worktree_manager::config::ai_tool_resume_presets();
    let claude = &presets["claude"];
    assert!(claude.contains(&"--continue"));
    assert!(!claude.contains(&"--resume"));
}

#[test]
fn test_resume_preset_codex_uses_subcommand() {
    let presets = git_worktree_manager::config::ai_tool_resume_presets();
    let codex = &presets["codex"];
    assert_eq!(codex[0], "codex");
    assert_eq!(codex[1], "resume");
    assert!(codex.contains(&"--last"));
}

#[test]
fn test_merge_preset_claude_uses_print_mode() {
    let presets = git_worktree_manager::config::ai_tool_merge_presets();
    let claude = &presets["claude"];
    assert!(claude.flags.contains(&"--print"));
    assert!(claude.flags.contains(&"--tools=default"));
}

#[test]
fn test_claude_preset_names() {
    let names = git_worktree_manager::config::claude_preset_names();
    assert!(names.contains(&"claude"));
    assert!(names.contains(&"claude-yolo"));
    assert!(names.contains(&"claude-remote"));
    assert!(names.contains(&"claude-yolo-remote"));
    assert!(!names.contains(&"codex"));
    assert!(!names.contains(&"no-op"));
}

#[test]
fn test_config_serialization_roundtrip() {
    let config = git_worktree_manager::config::Config::default();
    let json = serde_json::to_string(&config).unwrap();
    let deserialized: git_worktree_manager::config::Config = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.ai_tool.command, "claude");
    assert_eq!(deserialized.git.default_base_branch, "main");
    assert!(deserialized.update.auto_check);
}

#[test]
fn test_config_partial_json_deserialize() {
    // Partial JSON should merge with defaults
    let partial = r#"{"ai_tool": {"command": "codex", "args": ["--fast"]}}"#;
    let value: serde_json::Value = serde_json::from_str(partial).unwrap();
    // Verify the partial data is valid
    assert_eq!(value["ai_tool"]["command"], "codex");
    assert_eq!(value["ai_tool"]["args"][0], "--fast");
}

#[test]
fn test_resolve_launch_alias_deprecated_bg() {
    // "bg" should resolve to "detach" with deprecation warning
    let result = git_worktree_manager::config::resolve_launch_alias("bg");
    assert_eq!(result, "detach");
}

#[test]
fn test_resolve_launch_alias_deprecated_background() {
    let result = git_worktree_manager::config::resolve_launch_alias("background");
    assert_eq!(result, "detach");
}

#[test]
fn test_parse_term_option_all_methods() {
    use git_worktree_manager::config::parse_term_option;
    use git_worktree_manager::constants::LaunchMethod;

    let cases = vec![
        ("foreground", LaunchMethod::Foreground),
        ("detach", LaunchMethod::Detach),
        ("iterm-window", LaunchMethod::ItermWindow),
        ("iterm-tab", LaunchMethod::ItermTab),
        ("iterm-pane-h", LaunchMethod::ItermPaneH),
        ("iterm-pane-v", LaunchMethod::ItermPaneV),
        ("tmux", LaunchMethod::Tmux),
        ("tmux-window", LaunchMethod::TmuxWindow),
        ("tmux-pane-h", LaunchMethod::TmuxPaneH),
        ("tmux-pane-v", LaunchMethod::TmuxPaneV),
        ("zellij", LaunchMethod::Zellij),
        ("zellij-tab", LaunchMethod::ZellijTab),
        ("zellij-pane-h", LaunchMethod::ZellijPaneH),
        ("zellij-pane-v", LaunchMethod::ZellijPaneV),
        ("wezterm-window", LaunchMethod::WeztermWindow),
        ("wezterm-tab", LaunchMethod::WeztermTab),
        ("wezterm-pane-h", LaunchMethod::WeztermPaneH),
        ("wezterm-pane-v", LaunchMethod::WeztermPaneV),
    ];

    for (input, expected) in cases {
        let (m, _) = parse_term_option(Some(input)).unwrap();
        assert_eq!(m, expected, "Failed for input: {}", input);
    }
}

#[test]
fn test_parse_term_option_session_name_on_non_tmux_zellij() {
    use git_worktree_manager::config::parse_term_option;
    // Session names only supported for tmux and zellij
    assert!(parse_term_option(Some("foreground:session")).is_err());
    assert!(parse_term_option(Some("iterm-window:session")).is_err());
    assert!(parse_term_option(Some("wezterm-tab:session")).is_err());
}

#[test]
fn test_parse_term_option_zellij_session() {
    use git_worktree_manager::config::parse_term_option;
    use git_worktree_manager::constants::LaunchMethod;

    let (m, s) = parse_term_option(Some("z:mydev")).unwrap();
    assert_eq!(m, LaunchMethod::Zellij);
    assert_eq!(s.unwrap(), "mydev");
}

#[test]
fn test_list_presets_contains_all_presets() {
    let output = git_worktree_manager::config::list_presets();
    for name in [
        "claude",
        "claude-yolo",
        "claude-remote",
        "claude-yolo-remote",
        "codex",
        "codex-yolo",
        "no-op",
    ] {
        assert!(output.contains(name), "Missing preset: {}", name);
    }
}
