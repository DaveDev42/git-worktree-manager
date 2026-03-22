/// Hook execution system for git-worktree-manager.
///
/// Hooks allow users to run custom commands at lifecycle events.
/// Stored per-repository in .cwconfig.json.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use console::style;

use crate::error::{CwError, Result};

/// Valid hook events.
pub const HOOK_EVENTS: &[&str] = &[
    "worktree.pre_create",
    "worktree.post_create",
    "worktree.pre_delete",
    "worktree.post_delete",
    "merge.pre",
    "merge.post",
    "pr.pre",
    "pr.post",
    "resume.pre",
    "resume.post",
    "sync.pre",
    "sync.post",
];

/// Local config file name (stored in repository root).
const LOCAL_CONFIG_FILE: &str = ".cwconfig.json";

/// A single hook entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookEntry {
    pub id: String,
    pub command: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub description: String,
}

fn default_true() -> bool {
    true
}

/// Find the git repository root by walking up from start_path.
fn find_repo_root(start_path: Option<&Path>) -> Option<PathBuf> {
    let start = start_path
        .map(|p| p.to_path_buf())
        .or_else(|| std::env::current_dir().ok())?;

    let mut current = start.canonicalize().unwrap_or(start);
    loop {
        if current.join(".git").exists() {
            return Some(current);
        }
        if !current.pop() {
            break;
        }
    }
    None
}

/// Get the path to the local config file.
fn get_hooks_file_path(repo_root: Option<&Path>) -> Option<PathBuf> {
    let root = if let Some(r) = repo_root {
        r.to_path_buf()
    } else {
        find_repo_root(None)?
    };
    Some(root.join(LOCAL_CONFIG_FILE))
}

/// Load hooks configuration from the repository.
pub fn load_hooks_config(repo_root: Option<&Path>) -> HashMap<String, Vec<HookEntry>> {
    let hooks_file = match get_hooks_file_path(repo_root) {
        Some(p) if p.exists() => p,
        _ => return HashMap::new(),
    };

    let content = match std::fs::read_to_string(&hooks_file) {
        Ok(c) => c,
        Err(_) => return HashMap::new(),
    };

    let data: Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return HashMap::new(),
    };

    let hooks_obj = match data.get("hooks") {
        Some(Value::Object(m)) => m,
        _ => return HashMap::new(),
    };

    let mut result = HashMap::new();
    for (event, entries) in hooks_obj {
        if let Ok(hooks) = serde_json::from_value::<Vec<HookEntry>>(entries.clone()) {
            result.insert(event.clone(), hooks);
        }
    }
    result
}

/// Save hooks configuration.
pub fn save_hooks_config(
    hooks: &HashMap<String, Vec<HookEntry>>,
    repo_root: Option<&Path>,
) -> Result<()> {
    let root = if let Some(r) = repo_root {
        r.to_path_buf()
    } else {
        find_repo_root(None).ok_or_else(|| CwError::Hook("Not in a git repository".to_string()))?
    };

    let config_file = root.join(LOCAL_CONFIG_FILE);
    let data = serde_json::json!({ "hooks": hooks });
    let content = serde_json::to_string_pretty(&data)?;
    std::fs::write(&config_file, content)?;
    Ok(())
}

/// Generate a unique ID for a hook based on command hash.
fn generate_hook_id(command: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    command.hash(&mut hasher);
    format!("hook-{:08x}", hasher.finish() as u32)
}

/// Add a new hook for an event.
pub fn add_hook(
    event: &str,
    command: &str,
    hook_id: Option<&str>,
    description: Option<&str>,
) -> Result<String> {
    if !HOOK_EVENTS.contains(&event) {
        return Err(CwError::Hook(format!(
            "Invalid hook event: {}. Valid events: {}",
            event,
            HOOK_EVENTS.join(", ")
        )));
    }

    let mut hooks = load_hooks_config(None);
    let event_hooks = hooks.entry(event.to_string()).or_default();

    let id = hook_id
        .map(|s| s.to_string())
        .unwrap_or_else(|| generate_hook_id(command));

    // Check for duplicate
    if event_hooks.iter().any(|h| h.id == id) {
        return Err(CwError::Hook(format!(
            "Hook with ID '{}' already exists for event '{}'",
            id, event
        )));
    }

    event_hooks.push(HookEntry {
        id: id.clone(),
        command: command.to_string(),
        enabled: true,
        description: description.unwrap_or("").to_string(),
    });

    save_hooks_config(&hooks, None)?;
    Ok(id)
}

/// Remove a hook by event and ID.
pub fn remove_hook(event: &str, hook_id: &str) -> Result<()> {
    let mut hooks = load_hooks_config(None);
    let event_hooks = hooks
        .get_mut(event)
        .ok_or_else(|| CwError::Hook(format!("No hooks found for event '{}'", event)))?;

    let original_len = event_hooks.len();
    event_hooks.retain(|h| h.id != hook_id);

    if event_hooks.len() == original_len {
        return Err(CwError::Hook(format!(
            "Hook '{}' not found for event '{}'",
            hook_id, event
        )));
    }

    save_hooks_config(&hooks, None)?;
    println!("* Removed hook '{}' from {}", hook_id, event);
    Ok(())
}

/// Enable or disable a hook.
pub fn set_hook_enabled(event: &str, hook_id: &str, enabled: bool) -> Result<()> {
    let mut hooks = load_hooks_config(None);
    let event_hooks = hooks
        .get_mut(event)
        .ok_or_else(|| CwError::Hook(format!("No hooks found for event '{}'", event)))?;

    let hook = event_hooks
        .iter_mut()
        .find(|h| h.id == hook_id)
        .ok_or_else(|| {
            CwError::Hook(format!(
                "Hook '{}' not found for event '{}'",
                hook_id, event
            ))
        })?;

    hook.enabled = enabled;
    save_hooks_config(&hooks, None)?;

    let action = if enabled { "Enabled" } else { "Disabled" };
    println!("* {} hook '{}'", action, hook_id);
    Ok(())
}

/// Get hooks for a specific event.
pub fn get_hooks(event: &str, repo_root: Option<&Path>) -> Vec<HookEntry> {
    let hooks = load_hooks_config(repo_root);
    hooks.get(event).cloned().unwrap_or_default()
}

/// Run all enabled hooks for an event.
///
/// Pre-hooks (containing ".pre") abort the operation on failure.
/// Post-hooks log warnings but don't abort.
pub fn run_hooks(
    event: &str,
    context: &HashMap<String, String>,
    cwd: Option<&Path>,
    repo_root: Option<&Path>,
) -> Result<bool> {
    let hooks = get_hooks(event, repo_root);
    if hooks.is_empty() {
        return Ok(true);
    }

    let enabled: Vec<&HookEntry> = hooks.iter().filter(|h| h.enabled).collect();
    if enabled.is_empty() {
        return Ok(true);
    }

    let is_pre_hook = event.contains(".pre");

    eprintln!(
        "{} Running {} hook(s) for {}...",
        style("*").cyan().bold(),
        enabled.len(),
        style(event).yellow()
    );

    // Build environment
    let mut env: HashMap<String, String> = std::env::vars().collect();
    for (key, value) in context {
        env.insert(format!("CW_{}", key.to_uppercase()), value.clone());
    }

    let mut all_succeeded = true;

    for hook in enabled {
        let desc_suffix = if hook.description.is_empty() {
            String::new()
        } else {
            format!(" ({})", hook.description)
        };
        eprintln!(
            "  {} {}{}",
            style("Running:").dim(),
            style(&hook.id).bold(),
            style(desc_suffix).dim()
        );

        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = Command::new("cmd");
            c.args(["/C", &hook.command]);
            c
        } else {
            let mut c = Command::new("sh");
            c.args(["-c", &hook.command]);
            c
        };

        cmd.envs(&env);
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        match cmd.output() {
            Ok(output) => {
                if !output.status.success() {
                    all_succeeded = false;
                    let code = output.status.code().unwrap_or(-1);
                    eprintln!(
                        "  {} Hook '{}' failed (exit code {})",
                        style("x").red().bold(),
                        style(&hook.id).bold(),
                        code
                    );

                    let stderr = String::from_utf8_lossy(&output.stderr);
                    for line in stderr.lines().take(5) {
                        eprintln!("    {}", style(line).dim());
                    }

                    if is_pre_hook {
                        return Err(CwError::Hook(format!(
                            "Pre-hook '{}' failed with exit code {}. Operation aborted.",
                            hook.id, code
                        )));
                    }
                } else {
                    eprintln!(
                        "  {} Hook '{}' completed",
                        style("*").green().bold(),
                        style(&hook.id).bold()
                    );
                }
            }
            Err(e) => {
                all_succeeded = false;
                eprintln!(
                    "  {} Hook '{}' failed: {}",
                    style("x").red().bold(),
                    style(&hook.id).bold(),
                    e
                );
                if is_pre_hook {
                    return Err(CwError::Hook(format!(
                        "Pre-hook '{}' failed to execute: {}",
                        hook.id, e
                    )));
                }
            }
        }
    }

    if !all_succeeded && !is_pre_hook {
        eprintln!(
            "{} Some post-hooks failed. See output above.",
            style("Warning:").yellow().bold()
        );
    }

    Ok(all_succeeded)
}
