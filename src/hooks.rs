/// Hook execution system for claude-worktree.
///
/// Hooks allow users to run custom commands at lifecycle events.
/// Stored per-repository in .cwconfig.json.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use serde_json::Value;

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
        find_repo_root(None)
            .ok_or_else(|| CwError::Hook("Not in a git repository".to_string()))?
    };

    let config_file = root.join(LOCAL_CONFIG_FILE);
    let data = serde_json::json!({ "hooks": hooks });
    let content = serde_json::to_string_pretty(&data)?;
    std::fs::write(&config_file, content)?;
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
        "Running {} hook(s) for {}...",
        enabled.len(),
        event
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
        eprintln!("  Running: {}{}", hook.id, desc_suffix);

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
                    eprintln!("  x Hook '{}' failed (exit code {})", hook.id, code);

                    let stderr = String::from_utf8_lossy(&output.stderr);
                    for line in stderr.lines().take(5) {
                        eprintln!("    {}", line);
                    }

                    if is_pre_hook {
                        return Err(CwError::Hook(format!(
                            "Pre-hook '{}' failed with exit code {}. Operation aborted.",
                            hook.id, code
                        )));
                    }
                } else {
                    eprintln!("  * Hook '{}' completed", hook.id);
                }
            }
            Err(e) => {
                all_succeeded = false;
                eprintln!("  x Hook '{}' failed: {}", hook.id, e);
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
        eprintln!("Warning: Some post-hooks failed. See output above.");
    }

    Ok(all_succeeded)
}
