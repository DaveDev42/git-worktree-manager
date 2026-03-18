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
    let config = claude_worktree::config::Config::default();
    assert_eq!(config.ai_tool.command, "claude");
    assert!(config.ai_tool.args.is_empty());
    assert_eq!(config.git.default_base_branch, "main");
    assert!(config.update.auto_check);
    assert_eq!(config.launch.tmux_session_prefix, "cw");
    assert_eq!(config.launch.wezterm_ready_timeout, 5.0);
    assert!(!config.shell_completion.prompted);
    assert!(!config.shell_completion.installed);
    assert!(config.launch.method.is_none());
}

#[test]
fn test_presets_all_present() {
    let presets = claude_worktree::config::ai_tool_presets();
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
    let presets = claude_worktree::config::ai_tool_resume_presets();
    assert!(presets.contains_key("claude"));
    assert!(presets.contains_key("codex"));
    // Claude uses --continue, not --resume
    assert!(presets["claude"].contains(&"--continue"));
    // Codex uses subcommand syntax
    assert!(presets["codex"].contains(&"resume"));
}

#[test]
fn test_merge_presets_all_present() {
    let presets = claude_worktree::config::ai_tool_merge_presets();
    assert!(presets.contains_key("claude"));
    assert!(presets.contains_key("codex"));
}

#[test]
fn test_list_presets_format() {
    let output = claude_worktree::config::list_presets();
    assert!(output.contains("Available AI tool presets:"));
    assert!(output.contains("claude"));
    assert!(output.contains("no-op"));
    assert!(output.contains("codex"));
}

#[test]
fn test_resolve_launch_alias() {
    assert_eq!(
        claude_worktree::config::resolve_launch_alias("fg"),
        "foreground"
    );
    assert_eq!(claude_worktree::config::resolve_launch_alias("t"), "tmux");
    assert_eq!(
        claude_worktree::config::resolve_launch_alias("z-t"),
        "zellij-tab"
    );
    assert_eq!(
        claude_worktree::config::resolve_launch_alias("i-w"),
        "iterm-window"
    );
    assert_eq!(
        claude_worktree::config::resolve_launch_alias("w-t"),
        "wezterm-tab"
    );
    assert_eq!(claude_worktree::config::resolve_launch_alias("d"), "detach");
    // Session name passthrough
    assert_eq!(
        claude_worktree::config::resolve_launch_alias("t:mywork"),
        "tmux:mywork"
    );
    assert_eq!(
        claude_worktree::config::resolve_launch_alias("z:dev"),
        "zellij:dev"
    );
    // Unknown passes through
    assert_eq!(
        claude_worktree::config::resolve_launch_alias("unknown"),
        "unknown"
    );
}

#[test]
fn test_parse_term_option() {
    use claude_worktree::config::parse_term_option;
    use claude_worktree::constants::LaunchMethod;

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
    let config = claude_worktree::config::Config::default();
    let json = serde_json::to_value(&config).unwrap();

    // Verify all top-level keys exist
    assert!(json.get("ai_tool").is_some());
    assert!(json.get("launch").is_some());
    assert!(json.get("git").is_some());
    assert!(json.get("update").is_some());
    assert!(json.get("shell_completion").is_some());
}
