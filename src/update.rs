/// Auto-update check and self-upgrade via GitHub Releases.
///
use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::Command;

use console::style;
use serde::{Deserialize, Serialize};

use crate::constants::home_dir_or_fallback;

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const REPO_OWNER: &str = "DaveDev42";
const REPO_NAME: &str = "git-worktree-manager";

/// Cache for update check results.
#[derive(Debug, Serialize, Deserialize, Default)]
struct UpdateCache {
    last_check: String,
    latest_version: Option<String>,
}

fn get_cache_path() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(home_dir_or_fallback)
        .join("git-worktree-manager")
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

    if let Some(latest) = fetch_latest_version() {
        let cache = UpdateCache {
            last_check: today_str(),
            latest_version: Some(latest.clone()),
        };
        save_cache(&cache);

        if is_newer(&latest, CURRENT_VERSION) {
            eprintln!(
                "\ngit-worktree-manager {} is available (current: {})",
                latest, CURRENT_VERSION
            );
            eprintln!("Run 'gw upgrade' to update.\n");
        }
    } else {
        let cache = UpdateCache {
            last_check: today_str(),
            latest_version: None,
        };
        save_cache(&cache);
    }
}

/// Get a GitHub auth token from gh CLI if available.
fn gh_auth_token() -> Option<String> {
    Command::new("gh")
        .args(["auth", "token"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|t| !t.is_empty())
}

/// Fetch latest version string from GitHub Releases API.
/// Uses gh auth token if available to avoid unauthenticated rate limits (60/hr).
fn fetch_latest_version() -> Option<String> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/releases/latest",
        REPO_OWNER, REPO_NAME
    );

    let mut args = vec![
        "-s".to_string(),
        "-H".to_string(),
        "Accept: application/vnd.github+json".to_string(),
    ];

    if let Some(token) = gh_auth_token() {
        args.push("-H".to_string());
        args.push(format!("Authorization: Bearer {}", token));
    }

    args.push(url);

    let output = Command::new("curl").args(&args).output().ok()?;

    if !output.status.success() {
        return None;
    }

    let body = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&body).ok()?;
    let tag = json.get("tag_name")?.as_str()?;

    // Strip tag prefix: "v0.0.3" → "0.0.3"
    Some(tag.strip_prefix('v').unwrap_or(tag).to_string())
}

/// Compare version strings (simple semver).
fn is_newer(latest: &str, current: &str) -> bool {
    let parse = |s: &str| -> Vec<u32> { s.split('.').filter_map(|p| p.parse().ok()).collect() };
    let l = parse(latest);
    let c = parse(current);
    l > c
}

/// Detect if the binary was installed via Homebrew.
fn is_homebrew_install() -> bool {
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return false,
    };
    // Resolve symlinks to get the real path
    let real_path = match std::fs::canonicalize(&exe) {
        Ok(p) => p,
        Err(_) => exe,
    };
    let path_str = real_path.to_string_lossy();
    // Homebrew installs to /opt/homebrew/Cellar/... or /usr/local/Cellar/...
    path_str.contains("/Cellar/") || path_str.contains("/homebrew/")
}

/// Manual upgrade command — downloads and installs the latest version.
pub fn upgrade() {
    println!("git-worktree-manager v{}", CURRENT_VERSION);

    // Check for Homebrew installation
    if is_homebrew_install() {
        println!(
            "{}",
            style("Installed via Homebrew. Use brew to upgrade:").yellow()
        );
        println!("  brew upgrade git-worktree-manager");
        return;
    }

    let latest_version = match fetch_latest_version() {
        Some(v) => v,
        None => {
            println!(
                "{}",
                style("Could not check for updates. Check your internet connection.").red()
            );
            return;
        }
    };

    if !is_newer(&latest_version, CURRENT_VERSION) {
        println!("{}", style("Already up to date.").green());
        return;
    }

    println!(
        "New version available: {} → {}",
        style(format!("v{}", CURRENT_VERSION)).dim(),
        style(format!("v{}", latest_version)).green().bold()
    );

    // Non-interactive: just print the info
    if !std::io::stdin().is_terminal() {
        println!(
            "Download from: https://github.com/{}/{}/releases/latest",
            REPO_OWNER, REPO_NAME
        );
        return;
    }

    // Prompt user
    let confirm = dialoguer::Confirm::new()
        .with_prompt("Upgrade now?")
        .default(true)
        .interact()
        .unwrap_or(false);

    if !confirm {
        println!("Upgrade cancelled.");
        return;
    }

    // Use self_update to download and replace
    println!("Downloading and installing...");
    match self_update::backends::github::Update::configure()
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .bin_name("gw")
        .current_version(CURRENT_VERSION)
        .target_version_tag(&format!("v{}", latest_version))
        .show_download_progress(true)
        .no_confirm(true) // We already confirmed above
        .build()
        .and_then(|updater| updater.update())
    {
        Ok(status) => {
            // Also update the cw binary if it exists alongside gw
            update_companion_binary();
            println!(
                "{}",
                style(format!("Upgraded to v{}!", status.version()))
                    .green()
                    .bold()
            );
        }
        Err(e) => {
            println!("{}", style(format!("Upgrade failed: {}", e)).red());
            println!(
                "Download manually: https://github.com/{}/{}/releases/latest",
                REPO_OWNER, REPO_NAME
            );
        }
    }
}

/// Update the `cw` companion binary alongside `gw`.
///
/// self_update only replaces the running binary (gw). Since cw is the same
/// binary, we copy the newly installed gw to cw.
fn update_companion_binary() {
    let current_exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return,
    };
    let bin_dir = match current_exe.parent() {
        Some(d) => d,
        None => return,
    };

    let bin_ext = if cfg!(windows) { ".exe" } else { "" };
    let gw_path = bin_dir.join(format!("gw{}", bin_ext));
    let cw_path = bin_dir.join(format!("cw{}", bin_ext));

    if cw_path.exists() {
        let _ = std::fs::copy(&gw_path, &cw_path);
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

    #[test]
    fn test_is_homebrew_install() {
        // Current binary is not from Homebrew in test context
        assert!(!is_homebrew_install());
    }
}
