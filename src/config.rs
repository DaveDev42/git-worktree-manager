/// Configuration management for git-worktree-manager.
///
/// Supports multiple AI coding assistants with customizable commands.
/// Configuration stored in ~/.config/git-worktree-manager/config.json.
use std::collections::HashMap;
use std::path::{Path, PathBuf};

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
    #[serde(default)]
    pub hooks: HookConfig,
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GitConfig {
    // default_base_branch removed — auto-detected per repo via git::detect_default_branch()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HookConfig {
    #[serde(default)]
    pub post_new: Option<String>,
    #[serde(default)]
    pub pre_rm: Option<String>,
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
            git: GitConfig {},
            update: UpdateConfig { auto_check: true },
            shell_completion: ShellCompletionConfig {
                prompted: false,
                installed: false,
            },
            hooks: HookConfig::default(),
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

/// Load configuration with the repo-local `.cwconfig.json` (if any) deep-merged
/// over the global config. Layer order (lowest to highest precedence):
///
///   1. `Config::default()`
///   2. Global `~/.config/git-worktree-manager/config.json` (or legacy path)
///   3. Repo-local `.cwconfig.json` walked up from `cwd`
///
/// Returns `Config::default()` if no config files are found.
pub fn load_effective_config(cwd: &Path) -> Result<Config> {
    load_effective_config_with_global(cwd, &get_config_path())
}

/// Same as [`load_effective_config`] but accepts an explicit global config path.
/// Carved out so tests can drive the global layer without setting `HOME`.
pub fn load_effective_config_with_global(cwd: &Path, global_path: &Path) -> Result<Config> {
    let mut merged = serde_json::to_value(Config::default())?;

    // Layer 2: global file (or legacy fallback if explicit path absent).
    let global_value = if global_path.exists() {
        Some(read_json_value(global_path)?)
    } else if global_path == get_config_path().as_path() {
        // Only fall through to legacy when the caller passed the *default* path —
        // tests that pass an explicit synthetic path should not pick up the user's
        // real legacy file.
        let legacy = get_legacy_config_path();
        if legacy.exists() {
            Some(read_json_value(&legacy)?)
        } else {
            None
        }
    } else {
        None
    };
    if let Some(v) = global_value {
        merged = deep_merge(merged, v);
    }

    // Layer 3: repo-local `.cwconfig.json` walked up from cwd.
    if let Some(repo_value) = crate::repo_config::load_repo_config(cwd)? {
        merged = deep_merge(merged, repo_value);
    }

    serde_json::from_value(merged)
        .map_err(|e| CwError::Config(format!("invalid effective config: {}", e)))
}

fn read_json_value(path: &Path) -> Result<serde_json::Value> {
    let content = std::fs::read_to_string(path).map_err(|e| {
        CwError::Config(format!(
            "failed to load config from {}: {}",
            path.display(),
            e
        ))
    })?;
    serde_json::from_str(&content).map_err(|e| {
        CwError::Config(format!(
            "failed to parse config from {}: {}",
            path.display(),
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
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    get_ai_tool_command_for_cwd(&cwd)
}

/// Like [`get_ai_tool_command`] but resolves config from `cwd` so a repo-local `.cwconfig.json` can override the global `ai_tool` block.
pub fn get_ai_tool_command_for_cwd(cwd: &Path) -> Result<Vec<String>> {
    // Check environment variable first
    if let Ok(env_tool) = std::env::var("CW_AI_TOOL") {
        if env_tool.trim().is_empty() {
            return Ok(Vec::new());
        }
        return Ok(env_tool.split_whitespace().map(String::from).collect());
    }

    let config = load_effective_config(cwd)?;
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
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    get_ai_tool_resume_command_for_cwd(&cwd)
}

/// Like [`get_ai_tool_resume_command`] but resolves config from `cwd` so a repo-local `.cwconfig.json` can override the global `ai_tool` block.
pub fn get_ai_tool_resume_command_for_cwd(cwd: &Path) -> Result<Vec<String>> {
    if let Ok(env_tool) = std::env::var("CW_AI_TOOL") {
        if env_tool.trim().is_empty() {
            return Ok(Vec::new());
        }
        let mut parts: Vec<String> = env_tool.split_whitespace().map(String::from).collect();
        parts.push("--resume".to_string());
        return Ok(parts);
    }

    let config = load_effective_config(cwd)?;
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
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    get_ai_tool_merge_command_for_cwd(&cwd, prompt)
}

/// Like [`get_ai_tool_merge_command`] but resolves config from `cwd` so a repo-local `.cwconfig.json` can override the global `ai_tool` block.
pub fn get_ai_tool_merge_command_for_cwd(cwd: &Path, prompt: &str) -> Result<Vec<String>> {
    if let Ok(env_tool) = std::env::var("CW_AI_TOOL") {
        if env_tool.trim().is_empty() {
            return Ok(Vec::new());
        }
        let mut parts: Vec<String> = env_tool.split_whitespace().map(String::from).collect();
        parts.push(prompt.to_string());
        return Ok(parts);
    }

    let config = load_effective_config(cwd)?;
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

/// Get the AI tool command with an initial prompt for interactive delegation.
///
/// Appends the prompt as a positional argument so the AI tool starts in interactive mode
/// with the given task. For Claude Code: `claude "<prompt>"` starts interactive with initial prompt.
pub fn get_ai_tool_delegate_command(prompt: &str) -> Result<Vec<String>> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    get_ai_tool_delegate_command_for_cwd(&cwd, prompt)
}

/// Like [`get_ai_tool_delegate_command`] but resolves config from `cwd` so a repo-local `.cwconfig.json` can override the global `ai_tool` block.
pub fn get_ai_tool_delegate_command_for_cwd(cwd: &Path, prompt: &str) -> Result<Vec<String>> {
    let mut cmd = get_ai_tool_command_for_cwd(cwd)?;
    if cmd.is_empty() {
        return Ok(cmd);
    }
    cmd.push(prompt.to_string());
    Ok(cmd)
}

/// Check if the currently configured AI tool is Claude-based.
pub fn is_claude_tool() -> Result<bool> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    is_claude_tool_for_cwd(&cwd)
}

/// Like [`is_claude_tool`] but resolves config from `cwd` so a repo-local `.cwconfig.json` can override the global `ai_tool` block.
pub fn is_claude_tool_for_cwd(cwd: &Path) -> Result<bool> {
    if let Ok(env_tool) = std::env::var("CW_AI_TOOL") {
        let first_word = env_tool.split_whitespace().next().unwrap_or("");
        return Ok(first_word == "claude");
    }
    let config = load_effective_config(cwd)?;
    Ok(claude_preset_names().contains(&config.ai_tool.command.as_str()))
}

/// Resolve a launch method value (possibly an alias) to its display name.
pub fn resolve_launch_display_name(method: &str) -> String {
    let aliases = launch_method_aliases();
    let canonical = aliases.get(method).copied().unwrap_or(method);
    LaunchMethod::from_str_opt(canonical)
        .map(|m| format!("{} ({})", m.display_name(), method))
        .unwrap_or_else(|| method.to_string())
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
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    get_default_launch_method_for_cwd(&cwd)
}

/// Like [`get_default_launch_method`] but resolves config from `cwd` so a repo-local `.cwconfig.json` can override the global `launch` block.
pub fn get_default_launch_method_for_cwd(cwd: &Path) -> Result<LaunchMethod> {
    // 1. Environment variable
    if let Ok(env_val) = std::env::var("CW_LAUNCH_METHOD") {
        let resolved = resolve_launch_alias(&env_val);
        if let Some(method) = LaunchMethod::from_str_opt(&resolved) {
            return Ok(method);
        }
    }

    // 2. Config file
    let config = load_effective_config(cwd)?;
    if let Some(ref method) = config.launch.method {
        let resolved = resolve_launch_alias(method);
        if let Some(m) = LaunchMethod::from_str_opt(&resolved) {
            return Ok(m);
        }
    }

    Ok(LaunchMethod::Foreground)
}

/// Resolve a launch method honoring an optional CLI override.
///
/// Resolution order:
///   1. `term_override` (the `-T/--term` CLI flag), if `Some` — parsed via
///      [`parse_term_option`], so it understands aliases and the
///      `method:session-name` syntax.
///   2. `CW_LAUNCH_METHOD` env var.
///   3. `.cwconfig.json` (repo-local) `launch.method`.
///   4. global `~/.config/git-worktree-manager/config.json` `launch.method`.
///   5. [`LaunchMethod::Foreground`] (the default).
///
/// Returns `(method, session_name)` where `session_name` is `Some` only
/// when the override used the `method:session-name` syntax (env / config
/// paths can't carry a session name today).
pub fn resolve_term_option(
    term_override: Option<&str>,
    cwd: &Path,
) -> Result<(LaunchMethod, Option<String>)> {
    if let Some(value) = term_override {
        // parse_term_option also accepts None (for which it returns the
        // default-method path); we narrow to the Some-arm explicitly so
        // an empty string from the CLI doesn't silently fall through.
        return parse_term_option(Some(value));
    }
    let method = get_default_launch_method_for_cwd(cwd)?;
    Ok((method, None))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.ai_tool.command, "claude");
        assert!(config.ai_tool.args.is_empty());
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
}
