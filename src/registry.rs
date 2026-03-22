/// Global repository registry for cross-repo worktree management.
///
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::constants::home_dir_or_fallback;
use crate::error::Result;
use crate::git;

const REGISTRY_VERSION: u32 = 1;

/// Directories to skip during filesystem scan.
const SCAN_SKIP_DIRS: &[&str] = &[
    "node_modules",
    ".cache",
    ".npm",
    ".yarn",
    "__pycache__",
    ".venv",
    "venv",
    ".tox",
    ".nox",
    ".eggs",
    "dist",
    "build",
    ".git",
    "Library",
    ".Trash",
    ".local",
    "Applications",
    ".cargo",
    ".rustup",
    ".pyenv",
    ".nvm",
    ".rbenv",
    ".goenv",
    ".volta",
    "site-packages",
    ".mypy_cache",
    ".ruff_cache",
    ".pytest_cache",
    "coverage",
    ".next",
    ".nuxt",
    ".output",
    ".turbo",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoEntry {
    pub name: String,
    pub registered_at: String,
    pub last_seen: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Registry {
    pub version: u32,
    pub repositories: HashMap<String, RepoEntry>,
}

impl Default for Registry {
    fn default() -> Self {
        Self {
            version: REGISTRY_VERSION,
            repositories: HashMap::new(),
        }
    }
}

/// Get the path to the global registry file.
pub fn get_registry_path() -> PathBuf {
    home_dir_or_fallback()
        .join(".config")
        .join("git-worktree-manager")
        .join("registry.json")
}

/// Load the global registry from disk.
pub fn load_registry() -> Registry {
    let path = get_registry_path();
    if !path.exists() {
        return Registry::default();
    }

    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => Registry::default(),
    }
}

/// Save the global registry to disk.
pub fn save_registry(registry: &Registry) -> Result<()> {
    let path = get_registry_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(registry)?;
    std::fs::write(&path, content)?;
    Ok(())
}

/// Register a repository in the global registry.
pub fn register_repo(repo_path: &Path) -> Result<()> {
    let mut registry = load_registry();
    let key = repo_path
        .canonicalize()
        .unwrap_or_else(|_| repo_path.to_path_buf())
        .to_string_lossy()
        .to_string();

    let now = crate::session::chrono_now_iso_pub();

    if let Some(entry) = registry.repositories.get_mut(&key) {
        entry.last_seen = now;
    } else {
        let name = repo_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        registry.repositories.insert(
            key,
            RepoEntry {
                name,
                registered_at: now.clone(),
                last_seen: now,
            },
        );
    }

    save_registry(&registry)
}

/// Update the last_seen timestamp.
pub fn update_last_seen(repo_path: &Path) -> Result<()> {
    let mut registry = load_registry();
    let key = repo_path
        .canonicalize()
        .unwrap_or_else(|_| repo_path.to_path_buf())
        .to_string_lossy()
        .to_string();

    if let Some(entry) = registry.repositories.get_mut(&key) {
        entry.last_seen = crate::session::chrono_now_iso_pub();
        save_registry(&registry)?;
    }
    Ok(())
}

/// Prune registry entries for non-existent repositories.
pub fn prune_registry() -> Result<Vec<String>> {
    let mut registry = load_registry();
    let mut removed = Vec::new();

    let keys: Vec<String> = registry.repositories.keys().cloned().collect();
    for key in keys {
        let path = PathBuf::from(&key);
        if !path.exists() || !path.join(".git").exists() {
            registry.repositories.remove(&key);
            removed.push(key);
        }
    }

    if !removed.is_empty() {
        save_registry(&registry)?;
    }
    Ok(removed)
}

/// Get all registered repositories.
pub fn get_all_registered_repos() -> Vec<(String, PathBuf)> {
    let registry = load_registry();
    registry
        .repositories
        .iter()
        .map(|(path, entry)| (entry.name.clone(), PathBuf::from(path)))
        .collect()
}

/// Check if a path is a main git repository root (not a worktree).
fn is_git_repo(path: &Path) -> bool {
    path.join(".git").is_dir()
}

/// Check if a git repository has worktrees beyond the main one.
fn has_worktrees(repo_path: &Path) -> bool {
    git::git_command(
        &["worktree", "list", "--porcelain"],
        Some(repo_path),
        false,
        true,
    )
    .map(|r| {
        r.stdout
            .lines()
            .filter(|l| l.starts_with("worktree "))
            .count()
            > 1
    })
    .unwrap_or(false)
}

/// Scan filesystem for git repositories with worktrees.
pub fn scan_for_repos(base_dir: Option<&Path>, max_depth: usize) -> Vec<PathBuf> {
    let base = base_dir
        .map(|p| p.to_path_buf())
        .unwrap_or_else(home_dir_or_fallback);

    let base = crate::git::canonicalize_or(&base);

    let mut found = Vec::new();

    fn scan_recursive(current: &Path, depth: usize, max_depth: usize, found: &mut Vec<PathBuf>) {
        if depth > max_depth {
            return;
        }

        let entries = match std::fs::read_dir(current) {
            Ok(e) => e,
            Err(_) => return,
        };

        let mut sorted: Vec<_> = entries.flatten().collect();
        sorted.sort_by_key(|e| e.file_name());

        for entry in sorted {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            if name_str.starts_with('.') || SCAN_SKIP_DIRS.contains(&name_str.as_ref()) {
                continue;
            }

            if is_git_repo(&path) && has_worktrees(&path) {
                found.push(path);
                continue; // Don't recurse into git repos
            }

            scan_recursive(&path, depth + 1, max_depth, found);
        }
    }

    scan_recursive(&base, 0, max_depth, &mut found);
    found
}
