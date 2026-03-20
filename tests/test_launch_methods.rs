/// Comprehensive tests for launch methods, aliases, config parsing, and launcher modules.
///
/// Mirrors the Python test file tests/test_launch_methods.py.
use std::collections::HashMap;
use std::path::Path;

use git_worktree_manager::config::{
    ai_tool_merge_presets, ai_tool_presets, ai_tool_resume_presets, claude_preset_names,
    get_default_launch_method, is_claude_tool, parse_term_option, resolve_launch_alias,
};
use git_worktree_manager::constants::{
    launch_method_aliases, LaunchMethod, MAX_SESSION_NAME_LENGTH,
};
use git_worktree_manager::operations::launchers;

// =========================================================================
// Helper: save/restore an environment variable around a closure.
// =========================================================================

fn with_env_var<F: FnOnce()>(key: &str, value: Option<&str>, f: F) {
    let saved = std::env::var(key).ok();
    match value {
        Some(v) => std::env::set_var(key, v),
        None => std::env::remove_var(key),
    }
    f();
    match saved {
        Some(v) => std::env::set_var(key, v),
        None => std::env::remove_var(key),
    }
}

/// Run closure with multiple env vars cleared, then restore them.
fn with_clean_env<F: FnOnce()>(keys: &[&str], f: F) {
    let saved: Vec<(&str, Option<String>)> =
        keys.iter().map(|k| (*k, std::env::var(k).ok())).collect();
    for k in keys {
        std::env::remove_var(k);
    }
    f();
    for (k, v) in saved {
        match v {
            Some(val) => std::env::set_var(k, val),
            None => std::env::remove_var(k),
        }
    }
}

// =========================================================================
// TestLaunchMethodEnum
// =========================================================================

const ALL_VARIANTS: [LaunchMethod; 18] = [
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

#[test]
fn test_all_enum_values_exist_and_roundtrip() {
    // All 18 variants should roundtrip through as_str/from_str_opt.
    for variant in &ALL_VARIANTS {
        let s = variant.as_str();
        let parsed = LaunchMethod::from_str_opt(s);
        assert_eq!(parsed, Some(*variant), "Roundtrip failed for {:?}", variant);
    }
}

#[test]
fn test_enum_as_str_values() {
    assert_eq!(LaunchMethod::Foreground.as_str(), "foreground");
    assert_eq!(LaunchMethod::Detach.as_str(), "detach");
    assert_eq!(LaunchMethod::ItermWindow.as_str(), "iterm-window");
    assert_eq!(LaunchMethod::ItermTab.as_str(), "iterm-tab");
    assert_eq!(LaunchMethod::ItermPaneH.as_str(), "iterm-pane-h");
    assert_eq!(LaunchMethod::ItermPaneV.as_str(), "iterm-pane-v");
    assert_eq!(LaunchMethod::Tmux.as_str(), "tmux");
    assert_eq!(LaunchMethod::TmuxWindow.as_str(), "tmux-window");
    assert_eq!(LaunchMethod::TmuxPaneH.as_str(), "tmux-pane-h");
    assert_eq!(LaunchMethod::TmuxPaneV.as_str(), "tmux-pane-v");
    assert_eq!(LaunchMethod::Zellij.as_str(), "zellij");
    assert_eq!(LaunchMethod::ZellijTab.as_str(), "zellij-tab");
    assert_eq!(LaunchMethod::ZellijPaneH.as_str(), "zellij-pane-h");
    assert_eq!(LaunchMethod::ZellijPaneV.as_str(), "zellij-pane-v");
    assert_eq!(LaunchMethod::WeztermWindow.as_str(), "wezterm-window");
    assert_eq!(LaunchMethod::WeztermTab.as_str(), "wezterm-tab");
    assert_eq!(LaunchMethod::WeztermPaneH.as_str(), "wezterm-pane-h");
    assert_eq!(LaunchMethod::WeztermPaneV.as_str(), "wezterm-pane-v");
}

#[test]
fn test_from_str_opt_valid() {
    let valid_strings = [
        "foreground",
        "detach",
        "iterm-window",
        "iterm-tab",
        "iterm-pane-h",
        "iterm-pane-v",
        "tmux",
        "tmux-window",
        "tmux-pane-h",
        "tmux-pane-v",
        "zellij",
        "zellij-tab",
        "zellij-pane-h",
        "zellij-pane-v",
        "wezterm-window",
        "wezterm-tab",
        "wezterm-pane-h",
        "wezterm-pane-v",
    ];
    for s in &valid_strings {
        assert!(
            LaunchMethod::from_str_opt(s).is_some(),
            "Expected Some for '{}'",
            s
        );
    }
}

#[test]
fn test_from_str_opt_invalid() {
    assert_eq!(LaunchMethod::from_str_opt("invalid"), None);
    assert_eq!(LaunchMethod::from_str_opt(""), None);
    assert_eq!(LaunchMethod::from_str_opt("bg"), None);
    assert_eq!(LaunchMethod::from_str_opt("background"), None);
    assert_eq!(LaunchMethod::from_str_opt("FOREGROUND"), None);
    assert_eq!(LaunchMethod::from_str_opt("tmux_window"), None);
}

#[test]
fn test_display_trait() {
    assert_eq!(format!("{}", LaunchMethod::Foreground), "foreground");
    assert_eq!(format!("{}", LaunchMethod::ItermTab), "iterm-tab");
    assert_eq!(format!("{}", LaunchMethod::Tmux), "tmux");
    assert_eq!(format!("{}", LaunchMethod::ZellijPaneH), "zellij-pane-h");
    assert_eq!(format!("{}", LaunchMethod::WeztermTab), "wezterm-tab");
}

#[test]
fn test_all_launch_methods_in_aliases_are_valid() {
    let aliases = launch_method_aliases();
    for (alias, target) in &aliases {
        assert!(
            LaunchMethod::from_str_opt(target).is_some(),
            "Alias '{}' -> '{}' targets an invalid launch method",
            alias,
            target
        );
    }
}

#[test]
fn test_alias_count() {
    let aliases = launch_method_aliases();
    assert!(
        aliases.len() >= 16,
        "Expected at least 16 aliases, got {}",
        aliases.len()
    );
}

#[test]
fn test_max_session_name_length() {
    assert_eq!(MAX_SESSION_NAME_LENGTH, 50);
}

// =========================================================================
// TestAliasResolution
// =========================================================================

#[test]
fn test_simple_aliases() {
    assert_eq!(resolve_launch_alias("fg"), "foreground");
    assert_eq!(resolve_launch_alias("d"), "detach");
}

#[test]
fn test_iterm_aliases() {
    assert_eq!(resolve_launch_alias("i-w"), "iterm-window");
    assert_eq!(resolve_launch_alias("i-t"), "iterm-tab");
    assert_eq!(resolve_launch_alias("i-p-h"), "iterm-pane-h");
    assert_eq!(resolve_launch_alias("i-p-v"), "iterm-pane-v");
}

#[test]
fn test_tmux_aliases() {
    assert_eq!(resolve_launch_alias("t"), "tmux");
    assert_eq!(resolve_launch_alias("t-w"), "tmux-window");
    assert_eq!(resolve_launch_alias("t-p-h"), "tmux-pane-h");
    assert_eq!(resolve_launch_alias("t-p-v"), "tmux-pane-v");
}

#[test]
fn test_zellij_aliases() {
    assert_eq!(resolve_launch_alias("z"), "zellij");
    assert_eq!(resolve_launch_alias("z-t"), "zellij-tab");
    assert_eq!(resolve_launch_alias("z-p-h"), "zellij-pane-h");
    assert_eq!(resolve_launch_alias("z-p-v"), "zellij-pane-v");
}

#[test]
fn test_wezterm_aliases() {
    assert_eq!(resolve_launch_alias("w-w"), "wezterm-window");
    assert_eq!(resolve_launch_alias("w-t"), "wezterm-tab");
    assert_eq!(resolve_launch_alias("w-p-h"), "wezterm-pane-h");
    assert_eq!(resolve_launch_alias("w-p-v"), "wezterm-pane-v");
}

#[test]
fn test_session_name_aliases() {
    assert_eq!(resolve_launch_alias("t:mywork"), "tmux:mywork");
    assert_eq!(resolve_launch_alias("z:dev"), "zellij:dev");
    assert_eq!(resolve_launch_alias("t-w:session1"), "tmux-window:session1");
}

#[test]
fn test_no_alias_passthrough() {
    assert_eq!(resolve_launch_alias("tmux"), "tmux");
    assert_eq!(resolve_launch_alias("foreground"), "foreground");
    assert_eq!(resolve_launch_alias("unknown"), "unknown");
    assert_eq!(resolve_launch_alias("zellij-tab"), "zellij-tab");
    assert_eq!(resolve_launch_alias("wezterm-window"), "wezterm-window");
}

#[test]
fn test_all_aliases_have_valid_targets() {
    let aliases = launch_method_aliases();
    for (alias, target) in &aliases {
        assert!(
            LaunchMethod::from_str_opt(target).is_some(),
            "Alias '{}' maps to invalid target '{}'",
            alias,
            target
        );
    }
}

#[test]
fn test_deprecated_bg_alias() {
    // "bg" should resolve to "detach"
    let result = resolve_launch_alias("bg");
    assert_eq!(result, "detach");
}

#[test]
fn test_deprecated_background_alias() {
    let result = resolve_launch_alias("background");
    assert_eq!(result, "detach");
}

#[test]
fn test_session_name_with_deprecated() {
    // "bg:foo" should resolve to "detach:foo"
    let result = resolve_launch_alias("bg:foo");
    assert_eq!(result, "detach:foo");
}

#[test]
fn test_full_name_with_session() {
    assert_eq!(resolve_launch_alias("tmux:my-session"), "tmux:my-session");
    assert_eq!(
        resolve_launch_alias("zellij:my-session"),
        "zellij:my-session"
    );
}

// =========================================================================
// TestParseTermOption
// =========================================================================

#[test]
fn test_none_returns_default() {
    with_clean_env(&["CW_LAUNCH_METHOD"], || {
        let (method, session) = parse_term_option(None).unwrap();
        // Without env or config, should default to Foreground
        assert_eq!(method, LaunchMethod::Foreground);
        assert!(session.is_none());
    });
}

#[test]
fn test_parse_simple_aliases() {
    let (method, session) = parse_term_option(Some("i-t")).unwrap();
    assert_eq!(method, LaunchMethod::ItermTab);
    assert!(session.is_none());

    let (method, session) = parse_term_option(Some("t")).unwrap();
    assert_eq!(method, LaunchMethod::Tmux);
    assert!(session.is_none());

    let (method, session) = parse_term_option(Some("z")).unwrap();
    assert_eq!(method, LaunchMethod::Zellij);
    assert!(session.is_none());
}

#[test]
fn test_parse_full_names() {
    let (method, session) = parse_term_option(Some("foreground")).unwrap();
    assert_eq!(method, LaunchMethod::Foreground);
    assert!(session.is_none());

    let (method, session) = parse_term_option(Some("tmux-window")).unwrap();
    assert_eq!(method, LaunchMethod::TmuxWindow);
    assert!(session.is_none());

    let (method, session) = parse_term_option(Some("zellij-pane-v")).unwrap();
    assert_eq!(method, LaunchMethod::ZellijPaneV);
    assert!(session.is_none());

    let (method, session) = parse_term_option(Some("wezterm-tab")).unwrap();
    assert_eq!(method, LaunchMethod::WeztermTab);
    assert!(session.is_none());
}

#[test]
fn test_session_names_tmux() {
    let (method, session) = parse_term_option(Some("t:mywork")).unwrap();
    assert_eq!(method, LaunchMethod::Tmux);
    assert_eq!(session, Some("mywork".to_string()));
}

#[test]
fn test_session_names_zellij() {
    let (method, session) = parse_term_option(Some("z:dev")).unwrap();
    assert_eq!(method, LaunchMethod::Zellij);
    assert_eq!(session, Some("dev".to_string()));
}

#[test]
fn test_full_name_with_session_parse() {
    let (method, session) = parse_term_option(Some("tmux:my-session")).unwrap();
    assert_eq!(method, LaunchMethod::Tmux);
    assert_eq!(session, Some("my-session".to_string()));
}

#[test]
fn test_session_name_length_limit() {
    let long_name = "a".repeat(51);
    let input = format!("t:{}", long_name);
    let result = parse_term_option(Some(&input));
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Session name too long"),
        "Expected 'Session name too long', got: {}",
        err_msg
    );
}

#[test]
fn test_session_name_at_limit() {
    let name_at_limit = "a".repeat(50);
    let input = format!("t:{}", name_at_limit);
    let (method, session) = parse_term_option(Some(&input)).unwrap();
    assert_eq!(method, LaunchMethod::Tmux);
    assert_eq!(session, Some(name_at_limit));
}

#[test]
fn test_invalid_method() {
    let result = parse_term_option(Some("invalid-method"));
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Invalid launch method"),
        "Expected 'Invalid launch method', got: {}",
        err_msg
    );
}

#[test]
fn test_session_name_on_unsupported_iterm() {
    let result = parse_term_option(Some("i-w:mysession"));
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Session name not supported"),
        "Expected 'Session name not supported', got: {}",
        err_msg
    );
}

#[test]
fn test_session_name_on_unsupported_wezterm() {
    let result = parse_term_option(Some("wezterm-window:test"));
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Session name not supported"),
        "Expected 'Session name not supported', got: {}",
        err_msg
    );
}

#[test]
fn test_all_methods_parse() {
    for variant in &ALL_VARIANTS {
        let s = variant.as_str();
        let result = parse_term_option(Some(s));
        assert!(
            result.is_ok(),
            "Failed to parse '{}': {:?}",
            s,
            result.err()
        );
        let (method, _) = result.unwrap();
        assert_eq!(method, *variant, "Parsed method mismatch for '{}'", s);
    }
}

// =========================================================================
// TestGetDefaultLaunchMethod
// =========================================================================

#[test]
fn test_fallback_foreground() {
    with_clean_env(&["CW_LAUNCH_METHOD"], || {
        let method = get_default_launch_method().unwrap();
        // Without config file override, should be Foreground
        assert_eq!(method, LaunchMethod::Foreground);
    });
}

#[test]
fn test_env_override_iterm_tab() {
    with_env_var("CW_LAUNCH_METHOD", Some("i-t"), || {
        let method = get_default_launch_method().unwrap();
        assert_eq!(method, LaunchMethod::ItermTab);
    });
}

#[test]
fn test_env_override_tmux() {
    with_env_var("CW_LAUNCH_METHOD", Some("tmux"), || {
        let method = get_default_launch_method().unwrap();
        assert_eq!(method, LaunchMethod::Tmux);
    });
}

#[test]
fn test_env_override_full_name() {
    with_env_var("CW_LAUNCH_METHOD", Some("zellij-pane-h"), || {
        let method = get_default_launch_method().unwrap();
        assert_eq!(method, LaunchMethod::ZellijPaneH);
    });
}

#[test]
fn test_invalid_env_falls_through() {
    with_env_var("CW_LAUNCH_METHOD", Some("invalid"), || {
        // Invalid env value should fall through to config/default
        let method = get_default_launch_method().unwrap();
        assert_eq!(method, LaunchMethod::Foreground);
    });
}

// =========================================================================
// TestLauncherModuleStructure
// =========================================================================

#[test]
fn test_tmux_window_requires_tmux_session() {
    with_clean_env(&["TMUX"], || {
        let result = launchers::tmux::launch_window(Path::new("/tmp"), "echo test", "test-tool");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("requires running inside a tmux session"),
            "Expected tmux session error, got: {}",
            err_msg
        );
    });
}

#[test]
fn test_tmux_pane_requires_tmux_session() {
    with_clean_env(&["TMUX"], || {
        let result =
            launchers::tmux::launch_pane(Path::new("/tmp"), "echo test", "test-tool", true);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("requires running inside a tmux session"),
            "Expected tmux session error, got: {}",
            err_msg
        );
    });
}

#[test]
fn test_tmux_pane_vertical_requires_tmux_session() {
    with_clean_env(&["TMUX"], || {
        let result =
            launchers::tmux::launch_pane(Path::new("/tmp"), "echo test", "test-tool", false);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("requires running inside a tmux session"),
            "Expected tmux session error, got: {}",
            err_msg
        );
    });
}

#[test]
fn test_zellij_tab_requires_zellij_session() {
    with_clean_env(&["ZELLIJ"], || {
        let result = launchers::zellij::launch_tab(Path::new("/tmp"), "echo test", "test-tool");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("requires running inside a Zellij session"),
            "Expected Zellij session error, got: {}",
            err_msg
        );
    });
}

#[test]
fn test_zellij_pane_requires_zellij_session() {
    with_clean_env(&["ZELLIJ"], || {
        let result =
            launchers::zellij::launch_pane(Path::new("/tmp"), "echo test", "test-tool", true);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("requires running inside a Zellij session"),
            "Expected Zellij session error, got: {}",
            err_msg
        );
    });
}

#[test]
fn test_zellij_pane_vertical_requires_zellij_session() {
    with_clean_env(&["ZELLIJ"], || {
        let result =
            launchers::zellij::launch_pane(Path::new("/tmp"), "echo test", "test-tool", false);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("requires running inside a Zellij session"),
            "Expected Zellij session error, got: {}",
            err_msg
        );
    });
}

#[cfg(not(target_os = "macos"))]
#[test]
fn test_iterm_requires_macos() {
    let result = launchers::iterm::launch_window(Path::new("/tmp"), "echo test", "test-tool");
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("only work on macOS"),
        "Expected macOS-only error, got: {}",
        err_msg
    );
}

#[test]
fn test_session_name_on_unsupported_foreground() {
    let result = parse_term_option(Some("foreground:mysession"));
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Session name not supported"),
        "Expected 'Session name not supported', got: {}",
        err_msg
    );
}

// =========================================================================
// TestShellQuoteAndSessionName (tested through public API)
// =========================================================================

#[test]
fn test_session_name_max_length_constant() {
    assert_eq!(MAX_SESSION_NAME_LENGTH, 50);
}

#[test]
fn test_session_name_length_enforced_in_parse_tmux() {
    let long_name = "b".repeat(51);
    let input = format!("tmux:{}", long_name);
    let result = parse_term_option(Some(&input));
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Session name too long"));
}

#[test]
fn test_session_name_length_enforced_in_parse_zellij() {
    let long_name = "c".repeat(51);
    let input = format!("zellij:{}", long_name);
    let result = parse_term_option(Some(&input));
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Session name too long"));
}

#[test]
fn test_various_special_chars_in_session_name() {
    // Session names with dashes, underscores, dots should be fine
    let (method, session) = parse_term_option(Some("t:my-work_v2.0")).unwrap();
    assert_eq!(method, LaunchMethod::Tmux);
    assert_eq!(session, Some("my-work_v2.0".to_string()));
}

#[test]
fn test_session_colons_in_name() {
    // "t:a:b" should split_once on first colon: method="tmux", session="a:b"
    let (method, session) = parse_term_option(Some("t:a:b")).unwrap();
    assert_eq!(method, LaunchMethod::Tmux);
    assert_eq!(session, Some("a:b".to_string()));
}

#[test]
fn test_session_empty_name() {
    // "t:" -> empty session name, should still work (len 0 < 50)
    let (method, session) = parse_term_option(Some("t:")).unwrap();
    assert_eq!(method, LaunchMethod::Tmux);
    assert_eq!(session, Some("".to_string()));
}

#[test]
fn test_session_name_exactly_one_char() {
    let (method, session) = parse_term_option(Some("z:x")).unwrap();
    assert_eq!(method, LaunchMethod::Zellij);
    assert_eq!(session, Some("x".to_string()));
}

#[test]
fn test_session_name_at_exact_limit_zellij() {
    let name = "z".repeat(50);
    let input = format!("zellij:{}", name);
    let (method, session) = parse_term_option(Some(&input)).unwrap();
    assert_eq!(method, LaunchMethod::Zellij);
    assert_eq!(session, Some(name));
}

// =========================================================================
// TestSmartContinueDetection
// =========================================================================

#[test]
fn test_is_claude_tool_default() {
    with_clean_env(&["CW_AI_TOOL"], || {
        // Default config has command="claude", which is a Claude preset
        let result = is_claude_tool().unwrap();
        assert!(result, "Default config should identify as Claude tool");
    });
}

#[test]
fn test_claude_preset_names_content() {
    let names = claude_preset_names();
    assert!(
        names.contains(&"claude"),
        "claude_preset_names should contain 'claude'"
    );
    assert!(
        names.contains(&"claude-yolo"),
        "claude_preset_names should contain 'claude-yolo'"
    );
    assert!(
        names.contains(&"claude-remote"),
        "claude_preset_names should contain 'claude-remote'"
    );
    assert!(
        names.contains(&"claude-yolo-remote"),
        "claude_preset_names should contain 'claude-yolo-remote'"
    );
    // codex and no-op should NOT be Claude presets
    assert!(
        !names.contains(&"codex"),
        "claude_preset_names should NOT contain 'codex'"
    );
    assert!(
        !names.contains(&"no-op"),
        "claude_preset_names should NOT contain 'no-op'"
    );
}

#[test]
fn test_is_claude_tool_with_env_claude() {
    with_env_var("CW_AI_TOOL", Some("claude"), || {
        let result = is_claude_tool().unwrap();
        assert!(result, "CW_AI_TOOL=claude should be identified as Claude");
    });
}

#[test]
fn test_is_claude_tool_with_env_codex() {
    with_env_var("CW_AI_TOOL", Some("codex"), || {
        let result = is_claude_tool().unwrap();
        assert!(
            !result,
            "CW_AI_TOOL=codex should NOT be identified as Claude"
        );
    });
}

// =========================================================================
// TestPresets
// =========================================================================

#[test]
fn test_merge_presets_exist() {
    let presets = ai_tool_merge_presets();
    assert!(presets.contains_key("claude"));
    assert!(presets.contains_key("claude-yolo"));
    assert!(presets.contains_key("claude-remote"));
    assert!(presets.contains_key("claude-yolo-remote"));
    assert!(presets.contains_key("codex"));
    assert!(presets.contains_key("codex-yolo"));
}

#[test]
fn test_merge_presets_have_flags() {
    let presets = ai_tool_merge_presets();
    for (name, preset) in &presets {
        assert!(
            !preset.flags.is_empty(),
            "Merge preset '{}' should have flags",
            name
        );
    }
}

#[test]
fn test_all_resume_presets_have_continue_or_resume() {
    let presets = ai_tool_resume_presets();
    for (name, cmd) in &presets {
        let has_continue = cmd
            .iter()
            .any(|s| s.contains("continue") || s.contains("resume"));
        assert!(
            has_continue,
            "Resume preset '{}' should contain --continue or resume: {:?}",
            name, cmd
        );
    }
}

#[test]
fn test_no_op_preset_empty() {
    let presets = ai_tool_presets();
    let no_op = presets.get("no-op").unwrap();
    assert!(no_op.is_empty(), "no-op preset should have empty command");
}

#[test]
fn test_codex_resume_preset() {
    let presets = ai_tool_resume_presets();
    let codex = presets.get("codex").unwrap();
    assert_eq!(codex[0], "codex");
    assert!(
        codex.contains(&"resume"),
        "codex resume preset should contain 'resume'"
    );
    assert!(
        codex.contains(&"--last"),
        "codex resume preset should contain '--last'"
    );
}

// =========================================================================
// Additional edge case tests
// =========================================================================

#[test]
fn test_all_presets_have_corresponding_resume() {
    let presets = ai_tool_presets();
    let resume_presets = ai_tool_resume_presets();
    // Every non-"no-op" preset that has a command should ideally have a resume preset
    for (name, cmd) in &presets {
        if *name == "no-op" || cmd.is_empty() {
            continue;
        }
        assert!(
            resume_presets.contains_key(name),
            "Preset '{}' should have a corresponding resume preset",
            name
        );
    }
}

#[test]
fn test_alias_map_no_duplicate_aliases() {
    let aliases = launch_method_aliases();
    let mut seen: HashMap<&str, &str> = HashMap::new();
    for (alias, target) in &aliases {
        if let Some(prev_target) = seen.insert(alias, target) {
            panic!(
                "Duplicate alias '{}' mapping to both '{}' and '{}'",
                alias, prev_target, target
            );
        }
    }
}

#[test]
fn test_detach_alias_via_parse() {
    let (method, session) = parse_term_option(Some("d")).unwrap();
    assert_eq!(method, LaunchMethod::Detach);
    assert!(session.is_none());
}

#[test]
fn test_foreground_alias_via_parse() {
    let (method, session) = parse_term_option(Some("fg")).unwrap();
    assert_eq!(method, LaunchMethod::Foreground);
    assert!(session.is_none());
}

#[test]
fn test_deprecated_bg_via_parse() {
    // "bg" is deprecated, resolves to "detach"
    let (method, session) = parse_term_option(Some("bg")).unwrap();
    assert_eq!(method, LaunchMethod::Detach);
    assert!(session.is_none());
}

#[test]
fn test_session_name_on_unsupported_detach() {
    let result = parse_term_option(Some("detach:mysession"));
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Session name not supported"),
        "Expected 'Session name not supported', got: {}",
        err_msg
    );
}

#[test]
fn test_session_name_on_unsupported_iterm_tab() {
    let result = parse_term_option(Some("iterm-tab:mysession"));
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Session name not supported"),
        "Expected 'Session name not supported', got: {}",
        err_msg
    );
}

#[test]
fn test_session_name_on_unsupported_wezterm_pane() {
    let result = parse_term_option(Some("wezterm-pane-h:test"));
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Session name not supported"),
        "Expected 'Session name not supported', got: {}",
        err_msg
    );
}

#[test]
fn test_claude_preset_count() {
    let names = claude_preset_names();
    assert_eq!(
        names.len(),
        4,
        "Expected exactly 4 Claude presets, got {}",
        names.len()
    );
}

#[test]
fn test_total_preset_count() {
    let presets = ai_tool_presets();
    assert_eq!(
        presets.len(),
        7,
        "Expected 7 total presets, got {}",
        presets.len()
    );
}
