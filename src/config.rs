/// Configuration management for git-worktree-manager.
///
/// Supports multiple AI coding assistants with customizable commands.
/// Configuration stored in ~/.config/git-worktree-manager/config.json.
use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::constants::{
    home_dir_or_fallback, launch_method_aliases, LaunchMethod, MAX_SESSION_NAME_LENGTH,
};
use crate::error::{CwError, Result};

/// Typed configuration structure matching the JSON schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub ai_tool: AiToolConfig,
    pub launch: LaunchConfig,
    pub git: GitConfig,
    pub update: UpdateConfig,
    pub shell_completion: ShellCompletionConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiToolConfig {
    pub command: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchConfig {
    pub method: Option<String>,
    pub tmux_session_prefix: String,
    pub wezterm_ready_timeout: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitConfig {
    pub default_base_branch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateConfig {
    pub auto_check: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellCompletionConfig {
    pub prompted: bool,
    pub installed: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ai_tool: AiToolConfig {
                command: "claude".to_string(),
                args: Vec::new(),
            },
            launch: LaunchConfig {
                method: None,
                tmux_session_prefix: "gw".to_string(),
                wezterm_ready_timeout: 5.0,
            },
            git: GitConfig {
                default_base_branch: "main".to_string(),
            },
            update: UpdateConfig { auto_check: true },
            shell_completion: ShellCompletionConfig {
                prompted: false,
                installed: false,
            },
        }
    }
}

/// AI tool presets: preset name -> command parts.
pub fn ai_tool_presets() -> HashMap<&'static str, Vec<&'static str>> {
    HashMap::from([
        ("no-op", vec![]),
        ("claude", vec!["claude"]),
        (
            "claude-yolo",
            vec!["claude", "--dangerously-skip-permissions"],
        ),
        ("claude-remote", vec!["claude", "/remote-control"]),
        (
            "claude-yolo-remote",
            vec![
                "claude",
                "--dangerously-skip-permissions",
                "/remote-control",
            ],
        ),
        ("codex", vec!["codex"]),
        (
            "codex-yolo",
            vec!["codex", "--dangerously-bypass-approvals-and-sandbox"],
        ),
    ])
}

/// AI tool resume presets.
pub fn ai_tool_resume_presets() -> HashMap<&'static str, Vec<&'static str>> {
    HashMap::from([
        ("claude", vec!["claude", "--continue"]),
        (
            "claude-yolo",
            vec!["claude", "--dangerously-skip-permissions", "--continue"],
        ),
        (
            "claude-remote",
            vec!["claude", "--continue", "/remote-control"],
        ),
        (
            "claude-yolo-remote",
            vec![
                "claude",
                "--dangerously-skip-permissions",
                "--continue",
                "/remote-control",
            ],
        ),
        ("codex", vec!["codex", "resume", "--last"]),
        (
            "codex-yolo",
            vec![
                "codex",
                "resume",
                "--dangerously-bypass-approvals-and-sandbox",
                "--last",
            ],
        ),
    ])
}

/// Merge preset configuration.
#[derive(Debug)]
pub struct MergePreset {
    pub base_override: Option<Vec<&'static str>>,
    pub flags: Vec<&'static str>,
    pub prompt_position: PromptPosition,
}

#[derive(Debug)]
pub enum PromptPosition {
    End,
    Index(usize),
}

/// AI tool merge presets.
pub fn ai_tool_merge_presets() -> HashMap<&'static str, MergePreset> {
    HashMap::from([
        (
            "claude",
            MergePreset {
                base_override: None,
                flags: vec!["--print", "--tools=default"],
                prompt_position: PromptPosition::End,
            },
        ),
        (
            "claude-yolo",
            MergePreset {
                base_override: None,
                flags: vec!["--print", "--tools=default"],
                prompt_position: PromptPosition::End,
            },
        ),
        (
            "claude-remote",
            MergePreset {
                base_override: Some(vec!["claude"]),
                flags: vec!["--print", "--tools=default"],
                prompt_position: PromptPosition::End,
            },
        ),
        (
            "claude-yolo-remote",
            MergePreset {
                base_override: Some(vec!["claude", "--dangerously-skip-permissions"]),
                flags: vec!["--print", "--tools=default"],
                prompt_position: PromptPosition::End,
            },
        ),
        (
            "codex",
            MergePreset {
                base_override: None,
                flags: vec!["--non-interactive"],
                prompt_position: PromptPosition::End,
            },
        ),
        (
            "codex-yolo",
            MergePreset {
                base_override: None,
                flags: vec!["--non-interactive"],
                prompt_position: PromptPosition::End,
            },
        ),
    ])
}

/// Set of Claude-based preset names.
pub fn claude_preset_names() -> Vec<&'static str> {
    ai_tool_presets()
        .iter()
        .filter(|(_, v)| v.first().map(|&s| s == "claude").unwrap_or(false))
        .map(|(&k, _)| k)
        .collect()
}

// ---------------------------------------------------------------------------
// Config file I/O
// ---------------------------------------------------------------------------

/// Get the path to the configuration file.
pub fn get_config_path() -> PathBuf {
    let home = home_dir_or_fallback();
    home.join(".config")
        .join("git-worktree-manager")
        .join("config.json")
}

/// Deep merge: override takes precedence, nested dicts merged recursively.
fn deep_merge(base: Value, over: Value) -> Value {
    match (base, over) {
        (Value::Object(mut base_map), Value::Object(over_map)) => {
            for (key, over_val) in over_map {
                let merged = if let Some(base_val) = base_map.remove(&key) {
                    deep_merge(base_val, over_val)
                } else {
                    over_val
                };
                base_map.insert(key, merged);
            }
            Value::Object(base_map)
        }
        (_, over) => over,
    }
}

/// Get the path to the legacy Python configuration file.
fn get_legacy_config_path() -> PathBuf {
    let home = home_dir_or_fallback();
    home.join(".config")
        .join("claude-worktree")
        .join("config.json")
}

/// Load configuration from file, deep-merged with defaults.
/// Falls back to legacy Python config path if the new path doesn't exist.
pub fn load_config() -> Result<Config> {
    let config_path = get_config_path();

    let config_path = if config_path.exists() {
        config_path
    } else {
        let legacy = get_legacy_config_path();
        if legacy.exists() {
            legacy
        } else {
            return Ok(Config::default());
        }
    };

    let content = std::fs::read_to_string(&config_path).map_err(|e| {
        CwError::Config(format!(
            "Failed to load config from {}: {}",
            config_path.display(),
            e
        ))
    })?;

    let file_value: Value = serde_json::from_str(&content).map_err(|e| {
        CwError::Config(format!(
            "Failed to parse config from {}: {}",
            config_path.display(),
            e
        ))
    })?;

    let default_value = serde_json::to_value(Config::default())?;
    let merged = deep_merge(default_value, file_value);

    serde_json::from_value(merged).map_err(|e| {
        CwError::Config(format!(
            "Failed to deserialize config from {}: {}",
            config_path.display(),
            e
        ))
    })
}

/// Save configuration to file.
pub fn save_config(config: &Config) -> Result<()> {
    let config_path = get_config_path();
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let content = serde_json::to_string_pretty(config)?;
    std::fs::write(&config_path, content).map_err(|e| {
        CwError::Config(format!(
            "Failed to save config to {}: {}",
            config_path.display(),
            e
        ))
    })
}

/// Get the AI tool command to execute.
///
/// Priority: CW_AI_TOOL env > config file > default ("claude").
pub fn get_ai_tool_command() -> Result<Vec<String>> {
    // Check environment variable first
    if let Ok(env_tool) = std::env::var("CW_AI_TOOL") {
        if env_tool.trim().is_empty() {
            return Ok(Vec::new());
        }
        return Ok(env_tool.split_whitespace().map(String::from).collect());
    }

    let config = load_config()?;
    let command = &config.ai_tool.command;
    let args = &config.ai_tool.args;

    let presets = ai_tool_presets();
    if let Some(base_cmd) = presets.get(command.as_str()) {
        let mut cmd: Vec<String> = base_cmd.iter().map(|s| s.to_string()).collect();
        cmd.extend(args.iter().cloned());
        return Ok(cmd);
    }

    if command.trim().is_empty() {
        return Ok(Vec::new());
    }

    let mut cmd = vec![command.clone()];
    cmd.extend(args.iter().cloned());
    Ok(cmd)
}

/// Get the AI tool resume command.
pub fn get_ai_tool_resume_command() -> Result<Vec<String>> {
    if let Ok(env_tool) = std::env::var("CW_AI_TOOL") {
        if env_tool.trim().is_empty() {
            return Ok(Vec::new());
        }
        let mut parts: Vec<String> = env_tool.split_whitespace().map(String::from).collect();
        parts.push("--resume".to_string());
        return Ok(parts);
    }

    let config = load_config()?;
    let command = &config.ai_tool.command;
    let args = &config.ai_tool.args;

    if command.trim().is_empty() {
        return Ok(Vec::new());
    }

    let resume_presets = ai_tool_resume_presets();
    if let Some(resume_cmd) = resume_presets.get(command.as_str()) {
        let mut cmd: Vec<String> = resume_cmd.iter().map(|s| s.to_string()).collect();
        cmd.extend(args.iter().cloned());
        return Ok(cmd);
    }

    let presets = ai_tool_presets();
    if let Some(base_cmd) = presets.get(command.as_str()) {
        if base_cmd.is_empty() {
            return Ok(Vec::new());
        }
        let mut cmd: Vec<String> = base_cmd.iter().map(|s| s.to_string()).collect();
        cmd.extend(args.iter().cloned());
        cmd.push("--resume".to_string());
        return Ok(cmd);
    }

    let mut cmd = vec![command.clone()];
    cmd.extend(args.iter().cloned());
    cmd.push("--resume".to_string());
    Ok(cmd)
}

/// Get the AI tool merge command.
pub fn get_ai_tool_merge_command(prompt: &str) -> Result<Vec<String>> {
    if let Ok(env_tool) = std::env::var("CW_AI_TOOL") {
        if env_tool.trim().is_empty() {
            return Ok(Vec::new());
        }
        let mut parts: Vec<String> = env_tool.split_whitespace().map(String::from).collect();
        parts.push(prompt.to_string());
        return Ok(parts);
    }

    let config = load_config()?;
    let command = &config.ai_tool.command;
    let args = &config.ai_tool.args;

    if command.trim().is_empty() {
        return Ok(Vec::new());
    }

    let merge_presets = ai_tool_merge_presets();
    if let Some(preset) = merge_presets.get(command.as_str()) {
        let base_cmd: Vec<String> = if let Some(ref base_override) = preset.base_override {
            base_override.iter().map(|s| s.to_string()).collect()
        } else {
            let presets = ai_tool_presets();
            presets
                .get(command.as_str())
                .map(|v| v.iter().map(|s| s.to_string()).collect())
                .unwrap_or_else(|| vec![command.clone()])
        };

        let mut cmd_parts = base_cmd;
        cmd_parts.extend(args.iter().cloned());
        cmd_parts.extend(preset.flags.iter().map(|s| s.to_string()));

        match preset.prompt_position {
            PromptPosition::End => cmd_parts.push(prompt.to_string()),
            PromptPosition::Index(i) => cmd_parts.insert(i, prompt.to_string()),
        }

        return Ok(cmd_parts);
    }

    let presets = ai_tool_presets();
    if let Some(base_cmd) = presets.get(command.as_str()) {
        if base_cmd.is_empty() {
            return Ok(Vec::new());
        }
        let mut cmd: Vec<String> = base_cmd.iter().map(|s| s.to_string()).collect();
        cmd.extend(args.iter().cloned());
        cmd.push(prompt.to_string());
        return Ok(cmd);
    }

    let mut cmd = vec![command.clone()];
    cmd.extend(args.iter().cloned());
    cmd.push(prompt.to_string());
    Ok(cmd)
}

/// Check if the currently configured AI tool is Claude-based.
pub fn is_claude_tool() -> Result<bool> {
    if let Ok(env_tool) = std::env::var("CW_AI_TOOL") {
        let first_word = env_tool.split_whitespace().next().unwrap_or("");
        return Ok(first_word == "claude");
    }
    let config = load_config()?;
    Ok(claude_preset_names().contains(&config.ai_tool.command.as_str()))
}

/// Set the AI tool command in configuration.
pub fn set_ai_tool(tool: &str, args: Option<Vec<String>>) -> Result<()> {
    let mut config = load_config()?;
    config.ai_tool.command = tool.to_string();
    config.ai_tool.args = args.unwrap_or_default();
    save_config(&config)
}

/// Use a predefined AI tool preset.
pub fn use_preset(preset_name: &str) -> Result<()> {
    let presets = ai_tool_presets();
    if !presets.contains_key(preset_name) {
        let available: Vec<&str> = presets.keys().copied().collect();
        return Err(CwError::Config(format!(
            "Unknown preset: {}. Available: {}",
            preset_name,
            available.join(", ")
        )));
    }
    set_ai_tool(preset_name, None)
}

/// Reset configuration to defaults.
pub fn reset_config() -> Result<()> {
    save_config(&Config::default())
}

/// Get a configuration value by dot-separated key path.
pub fn get_config_value(key_path: &str) -> Result<()> {
    let config = load_config()?;
    let json = serde_json::to_value(&config)?;

    let keys: Vec<&str> = key_path.split('.').collect();
    let mut current = &json;
    for &key in &keys {
        current = current
            .get(key)
            .ok_or_else(|| CwError::Config(format!("Unknown config key: {}", key_path)))?;
    }

    match current {
        serde_json::Value::String(s) => println!("{}", s),
        serde_json::Value::Bool(b) => println!("{}", b),
        serde_json::Value::Number(n) => println!("{}", n),
        serde_json::Value::Null => println!("null"),
        other => println!(
            "{}",
            serde_json::to_string_pretty(other).unwrap_or_default()
        ),
    }

    Ok(())
}

/// Set a configuration value by dot-separated key path.
pub fn set_config_value(key_path: &str, value: &str) -> Result<()> {
    let mut config = load_config()?;
    let mut json = serde_json::to_value(&config)?;

    let keys: Vec<&str> = key_path.split('.').collect();

    // Convert string boolean values
    let json_value: Value = match value.to_lowercase().as_str() {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        _ => {
            // Try to parse as number
            if let Ok(n) = value.parse::<f64>() {
                serde_json::Number::from_f64(n)
                    .map(Value::Number)
                    .unwrap_or(Value::String(value.to_string()))
            } else {
                Value::String(value.to_string())
            }
        }
    };

    // Navigate to parent and set value
    let mut current = &mut json;
    for &key in &keys[..keys.len() - 1] {
        if !current.is_object() {
            return Err(CwError::Config(format!(
                "Invalid config path: {}",
                key_path
            )));
        }
        current = current
            .as_object_mut()
            .ok_or_else(|| CwError::Config(format!("Invalid config path: {}", key_path)))?
            .entry(key)
            .or_insert(Value::Object(serde_json::Map::new()));
    }

    if let Some(obj) = current.as_object_mut() {
        obj.insert(keys[keys.len() - 1].to_string(), json_value);
    } else {
        return Err(CwError::Config(format!(
            "Invalid config path: {}",
            key_path
        )));
    }

    // Deserialize back to Config and save
    config = serde_json::from_value(json)
        .map_err(|e| CwError::Config(format!("Invalid config value: {}", e)))?;
    save_config(&config)
}

/// Get a formatted string of the current configuration.
pub fn show_config() -> Result<String> {
    let config = load_config()?;
    let mut lines = Vec::new();

    lines.push("Current configuration:".to_string());
    lines.push(String::new());
    lines.push(format!("  AI Tool: {}", config.ai_tool.command));

    if !config.ai_tool.args.is_empty() {
        lines.push(format!("    Args: {}", config.ai_tool.args.join(" ")));
    }

    let cmd = get_ai_tool_command()?;
    lines.push(format!("    Effective command: {}", cmd.join(" ")));
    lines.push(String::new());

    if let Some(ref method) = config.launch.method {
        lines.push(format!("  Launch method: {}", method));
    } else {
        lines.push("  Launch method: foreground (default)".to_string());
    }

    lines.push(format!(
        "  Default base branch: {}",
        config.git.default_base_branch
    ));
    lines.push(String::new());
    lines.push(format!("Config file: {}", get_config_path().display()));

    Ok(lines.join("\n"))
}

/// Get a formatted list of available presets.
pub fn list_presets() -> String {
    let presets = ai_tool_presets();
    let mut lines = vec!["Available AI tool presets:".to_string(), String::new()];

    let mut preset_names: Vec<&str> = presets.keys().copied().collect();
    preset_names.sort();

    for name in preset_names {
        let cmd = presets[name].join(" ");
        lines.push(format!("  {:<20} -> {}", name, cmd));
    }

    lines.join("\n")
}

// ---------------------------------------------------------------------------
// Shell completion prompt
// ---------------------------------------------------------------------------

/// Check if shell integration (gw-cd) is already installed in the user's profile.
fn is_shell_integration_installed() -> bool {
    let home = home_dir_or_fallback();
    let shell_env = std::env::var("SHELL").unwrap_or_default();

    let profile_path = if shell_env.contains("zsh") {
        home.join(".zshrc")
    } else if shell_env.contains("bash") {
        home.join(".bashrc")
    } else if shell_env.contains("fish") {
        home.join(".config").join("fish").join("config.fish")
    } else {
        return false;
    };

    if let Ok(content) = std::fs::read_to_string(&profile_path) {
        content.contains("gw _shell-function") || content.contains("gw-cd")
    } else {
        false
    }
}

/// Prompt user to set up shell integration on first run.
///
/// Shows a one-time hint if:
/// - Shell integration is not already installed
/// - User has not been prompted before
///
/// Updates `shell_completion.prompted` in config after showing.
pub fn prompt_shell_completion_setup() {
    let config = match load_config() {
        Ok(c) => c,
        Err(_) => return,
    };

    if config.shell_completion.prompted || config.shell_completion.installed {
        return;
    }

    if is_shell_integration_installed() {
        // Already installed — mark both flags and skip
        let mut config = config;
        config.shell_completion.prompted = true;
        config.shell_completion.installed = true;
        let _ = save_config(&config);
        return;
    }

    // Show one-time hint
    eprintln!(
        "\n{} Shell integration (gw-cd + tab completion) is not set up.",
        console::style("Tip:").cyan().bold()
    );
    eprintln!(
        "     Run {} to enable directory navigation and completions.\n",
        console::style("gw shell-setup").cyan()
    );

    // Mark as prompted
    let mut config = config;
    config.shell_completion.prompted = true;
    let _ = save_config(&config);
}

// ---------------------------------------------------------------------------
// Launch method configuration
// ---------------------------------------------------------------------------

/// Resolve launch method alias to full name.
pub fn resolve_launch_alias(value: &str) -> String {
    let deprecated: HashMap<&str, &str> =
        HashMap::from([("bg", "detach"), ("background", "detach")]);
    let aliases = launch_method_aliases();

    // Handle session name suffix (e.g., "t:mysession")
    if let Some((prefix, suffix)) = value.split_once(':') {
        let resolved_prefix = if let Some(&new) = deprecated.get(prefix) {
            eprintln!(
                "Warning: '{}' is deprecated. Use '{}' instead.",
                prefix, new
            );
            new.to_string()
        } else {
            aliases
                .get(prefix)
                .map(|s| s.to_string())
                .unwrap_or_else(|| prefix.to_string())
        };
        return format!("{}:{}", resolved_prefix, suffix);
    }

    if let Some(&new) = deprecated.get(value) {
        eprintln!("Warning: '{}' is deprecated. Use '{}' instead.", value, new);
        return new.to_string();
    }

    aliases
        .get(value)
        .map(|s| s.to_string())
        .unwrap_or_else(|| value.to_string())
}

/// Parse --term option value.
///
/// Returns (LaunchMethod, optional_session_name).
pub fn parse_term_option(term_value: Option<&str>) -> Result<(LaunchMethod, Option<String>)> {
    let term_value = match term_value {
        Some(v) => v,
        None => return Ok((get_default_launch_method()?, None)),
    };

    let resolved = resolve_launch_alias(term_value);

    if let Some((method_str, session_name)) = resolved.split_once(':') {
        let method = LaunchMethod::from_str_opt(method_str)
            .ok_or_else(|| CwError::Config(format!("Invalid launch method: {}", method_str)))?;

        if matches!(method, LaunchMethod::Tmux | LaunchMethod::Zellij) {
            if session_name.len() > MAX_SESSION_NAME_LENGTH {
                return Err(CwError::Config(format!(
                    "Session name too long (max {} chars): {}",
                    MAX_SESSION_NAME_LENGTH, session_name
                )));
            }
            return Ok((method, Some(session_name.to_string())));
        } else {
            return Err(CwError::Config(format!(
                "Session name not supported for {}",
                method_str
            )));
        }
    }

    let method = LaunchMethod::from_str_opt(&resolved)
        .ok_or_else(|| CwError::Config(format!("Invalid launch method: {}", term_value)))?;
    Ok((method, None))
}

/// Get default launch method from config or environment.
pub fn get_default_launch_method() -> Result<LaunchMethod> {
    // 1. Environment variable
    if let Ok(env_val) = std::env::var("CW_LAUNCH_METHOD") {
        let resolved = resolve_launch_alias(&env_val);
        if let Some(method) = LaunchMethod::from_str_opt(&resolved) {
            return Ok(method);
        }
    }

    // 2. Config file
    let config = load_config()?;
    if let Some(ref method) = config.launch.method {
        let resolved = resolve_launch_alias(method);
        if let Some(m) = LaunchMethod::from_str_opt(&resolved) {
            return Ok(m);
        }
    }

    Ok(LaunchMethod::Foreground)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.ai_tool.command, "claude");
        assert!(config.ai_tool.args.is_empty());
        assert_eq!(config.git.default_base_branch, "main");
        assert!(config.update.auto_check);
    }

    #[test]
    fn test_resolve_launch_alias() {
        assert_eq!(resolve_launch_alias("fg"), "foreground");
        assert_eq!(resolve_launch_alias("t"), "tmux");
        assert_eq!(resolve_launch_alias("z-t"), "zellij-tab");
        assert_eq!(resolve_launch_alias("t:mywork"), "tmux:mywork");
        assert_eq!(resolve_launch_alias("foreground"), "foreground");
    }

    #[test]
    fn test_parse_term_option() {
        let (method, session) = parse_term_option(Some("t")).unwrap();
        assert_eq!(method, LaunchMethod::Tmux);
        assert!(session.is_none());

        let (method, session) = parse_term_option(Some("t:mywork")).unwrap();
        assert_eq!(method, LaunchMethod::Tmux);
        assert_eq!(session.unwrap(), "mywork");

        let (method, session) = parse_term_option(Some("i-t")).unwrap();
        assert_eq!(method, LaunchMethod::ItermTab);
        assert!(session.is_none());
    }

    #[test]
    fn test_preset_names() {
        let presets = ai_tool_presets();
        assert!(presets.contains_key("claude"));
        assert!(presets.contains_key("no-op"));
        assert!(presets.contains_key("codex"));
        assert_eq!(presets["no-op"].len(), 0);
        assert_eq!(presets["claude"], vec!["claude"]);
    }

    #[test]
    fn test_list_presets_format() {
        let output = list_presets();
        assert!(output.contains("Available AI tool presets:"));
        assert!(output.contains("claude"));
        assert!(output.contains("no-op"));
    }
}
