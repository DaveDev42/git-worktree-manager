/// Auto-update check via GitHub Releases.
///
/// Mirrors src/claude_worktree/update.py — but uses GitHub API instead of PyPI.
use std::path::PathBuf;

use serde::{Deserialize, Serialize};


const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Cache for update check results.
#[derive(Debug, Serialize, Deserialize, Default)]
struct UpdateCache {
    last_check: String,
    latest_version: Option<String>,
}

fn get_cache_path() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
        .join("claude-worktree")
        .join("update_check.json")
}

fn load_cache() -> UpdateCache {
    let path = get_cache_path();
    if !path.exists() {
        return UpdateCache::default();
    }
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_default()
}

fn save_cache(cache: &UpdateCache) {
    let path = get_cache_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(content) = serde_json::to_string_pretty(cache) {
        let _ = std::fs::write(&path, content);
    }
}

fn today_str() -> String {
    // Simple date string YYYY-MM-DD
    crate::session::chrono_now_iso_pub()
        .split('T')
        .next()
        .unwrap_or("")
        .to_string()
}

fn should_check() -> bool {
    let config = crate::config::load_config().unwrap_or_default();
    if !config.update.auto_check {
        return false;
    }
    let cache = load_cache();
    cache.last_check != today_str()
}

/// Check for updates (called on startup, non-blocking).
pub fn check_for_update_if_needed() {
    if !should_check() {
        return;
    }

    // Check in background — fetch latest from GitHub API
    if let Some(latest) = fetch_latest_version() {
        let cache = UpdateCache {
            last_check: today_str(),
            latest_version: Some(latest.clone()),
        };
        save_cache(&cache);

        if is_newer(&latest, CURRENT_VERSION) {
            eprintln!(
                "\nclaude-worktree {} is available (current: {})",
                latest, CURRENT_VERSION
            );
            eprintln!("Run 'cw upgrade' to update.\n");
        }
    } else {
        // Save cache even on failure to avoid retrying today
        let cache = UpdateCache {
            last_check: today_str(),
            latest_version: None,
        };
        save_cache(&cache);
    }
}

/// Fetch latest version from GitHub Releases API.
fn fetch_latest_version() -> Option<String> {
    // Use ureq or a simple curl for the HTTP request
    // For now, use subprocess to call curl (avoids adding ureq dependency)
    let output = std::process::Command::new("curl")
        .args([
            "-s",
            "-H",
            "Accept: application/vnd.github+json",
            "https://api.github.com/repos/DaveDev42/claude-worktree-rs/releases/latest",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let body = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&body).ok()?;
    let tag = json.get("tag_name")?.as_str()?;

    // Strip "v" prefix
    Some(tag.strip_prefix('v').unwrap_or(tag).to_string())
}

/// Compare version strings (simple semver).
fn is_newer(latest: &str, current: &str) -> bool {
    let parse = |s: &str| -> Vec<u32> {
        s.split('.')
            .filter_map(|p| p.parse().ok())
            .collect()
    };
    let l = parse(latest);
    let c = parse(current);
    l > c
}

/// Manual upgrade command.
pub fn upgrade() {
    println!("claude-worktree v{}", CURRENT_VERSION);

    if let Some(latest) = fetch_latest_version() {
        if is_newer(&latest, CURRENT_VERSION) {
            println!("New version available: v{}", latest);
            println!("\nTo upgrade:");
            println!("  Download from: https://github.com/DaveDev42/claude-worktree-rs/releases/latest");

            #[cfg(target_os = "macos")]
            println!("  Or: brew upgrade claude-worktree");
        } else {
            println!("Already up to date.");
        }
    } else {
        println!("Could not check for updates. Check your internet connection.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_newer() {
        assert!(is_newer("0.2.0", "0.1.0"));
        assert!(is_newer("1.0.0", "0.10.0"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.2.0"));
    }
}
