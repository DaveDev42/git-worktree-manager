/// Auto-update check and self-upgrade via GitHub Releases.
///
use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::Command;

use console::style;
use serde::{Deserialize, Serialize};

use crate::constants::home_dir_or_fallback;

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const GITHUB_REPO: &str = "DaveDev42/git-worktree-manager";

/// Cache for update check results.
#[derive(Debug, Serialize, Deserialize, Default)]
struct UpdateCache {
    last_check: String,
    latest_version: Option<String>,
}

/// Info about the latest release from GitHub.
struct ReleaseInfo {
    version: String,
    tag_name: String,
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

    if let Some(info) = fetch_latest_release() {
        let cache = UpdateCache {
            last_check: today_str(),
            latest_version: Some(info.version.clone()),
        };
        save_cache(&cache);

        if is_newer(&info.version, CURRENT_VERSION) {
            eprintln!(
                "\ngit-worktree-manager {} is available (current: {})",
                info.version, CURRENT_VERSION
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

/// Fetch latest release info from GitHub Releases API.
fn fetch_latest_release() -> Option<ReleaseInfo> {
    let output = Command::new("curl")
        .args([
            "-s",
            "-H",
            "Accept: application/vnd.github+json",
            &format!(
                "https://api.github.com/repos/{}/releases/latest",
                GITHUB_REPO
            ),
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let body = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&body).ok()?;
    let tag_name = json.get("tag_name")?.as_str()?.to_string();

    // Extract version: "git-worktree-manager-v0.0.2" → "0.0.2", or "v0.0.1" → "0.0.1"
    let version = tag_name
        .strip_prefix("git-worktree-manager-v")
        .or_else(|| tag_name.strip_prefix('v'))
        .unwrap_or(&tag_name)
        .to_string();

    Some(ReleaseInfo { version, tag_name })
}

/// Compare version strings (simple semver).
fn is_newer(latest: &str, current: &str) -> bool {
    let parse = |s: &str| -> Vec<u32> { s.split('.').filter_map(|p| p.parse().ok()).collect() };
    let l = parse(latest);
    let c = parse(current);
    l > c
}

/// Detect the current platform's target triple for asset downloads.
fn current_target() -> Option<&'static str> {
    #[cfg(all(target_arch = "x86_64", target_os = "linux"))]
    {
        Some("x86_64-unknown-linux-musl")
    }
    #[cfg(all(target_arch = "aarch64", target_os = "linux"))]
    {
        Some("aarch64-unknown-linux-musl")
    }
    #[cfg(all(target_arch = "x86_64", target_os = "macos"))]
    {
        Some("x86_64-apple-darwin")
    }
    #[cfg(all(target_arch = "aarch64", target_os = "macos"))]
    {
        Some("aarch64-apple-darwin")
    }
    #[cfg(all(target_arch = "x86_64", target_os = "windows"))]
    {
        Some("x86_64-pc-windows-msvc")
    }
    #[cfg(not(any(
        all(target_arch = "x86_64", target_os = "linux"),
        all(target_arch = "aarch64", target_os = "linux"),
        all(target_arch = "x86_64", target_os = "macos"),
        all(target_arch = "aarch64", target_os = "macos"),
        all(target_arch = "x86_64", target_os = "windows"),
    )))]
    {
        None
    }
}

/// Download and extract the release archive, returning the temp directory.
fn download_and_extract(tag_name: &str, target: &str) -> Result<PathBuf, String> {
    let ext = if cfg!(windows) { "zip" } else { "tar.gz" };
    let asset_name = format!("gw-{}.{}", target, ext);
    let url = format!(
        "https://github.com/{}/releases/download/{}/{}",
        GITHUB_REPO, tag_name, asset_name
    );

    let tmp_dir = std::env::temp_dir().join(format!("gw-upgrade-{}", std::process::id()));
    std::fs::create_dir_all(&tmp_dir).map_err(|e| format!("Failed to create temp dir: {}", e))?;

    let archive_path = tmp_dir.join(&asset_name);

    // Download with curl
    println!("Downloading {}...", asset_name);
    let status = Command::new("curl")
        .args(["-sL", "-o"])
        .arg(&archive_path)
        .arg(&url)
        .status()
        .map_err(|e| format!("Failed to run curl: {}", e))?;

    if !status.success() {
        let _ = std::fs::remove_dir_all(&tmp_dir);
        return Err(format!("Failed to download from {}", url));
    }

    // Verify the downloaded file is not an error page
    let file_size = std::fs::metadata(&archive_path)
        .map(|m| m.len())
        .unwrap_or(0);
    if file_size < 1024 {
        let _ = std::fs::remove_dir_all(&tmp_dir);
        return Err("Downloaded file is too small — asset may not exist for this platform".into());
    }

    // Extract
    println!("Extracting...");
    if cfg!(windows) {
        let status = Command::new("tar")
            .args(["-xf"])
            .arg(&archive_path)
            .arg("-C")
            .arg(&tmp_dir)
            .status()
            .map_err(|e| format!("Failed to extract zip: {}", e))?;
        if !status.success() {
            let _ = std::fs::remove_dir_all(&tmp_dir);
            return Err("Failed to extract archive".into());
        }
    } else {
        let status = Command::new("tar")
            .args(["xzf"])
            .arg(&archive_path)
            .arg("-C")
            .arg(&tmp_dir)
            .status()
            .map_err(|e| format!("Failed to extract tar.gz: {}", e))?;
        if !status.success() {
            let _ = std::fs::remove_dir_all(&tmp_dir);
            return Err("Failed to extract archive".into());
        }
    }

    Ok(tmp_dir)
}

/// Replace the current binary with the new one.
fn self_replace(tmp_dir: &std::path::Path) -> Result<(), String> {
    let current_exe =
        std::env::current_exe().map_err(|e| format!("Cannot determine current exe: {}", e))?;
    let current_dir = current_exe
        .parent()
        .ok_or("Cannot determine install directory")?;

    // Replace both gw and cw binaries
    let bin_ext = if cfg!(windows) { ".exe" } else { "" };
    for bin_name in &["gw", "cw"] {
        let src = tmp_dir.join(format!("{}{}", bin_name, bin_ext));
        let dest = current_dir.join(format!("{}{}", bin_name, bin_ext));

        if !src.exists() {
            // cw might not be in the archive if not built
            if *bin_name == "cw" {
                continue;
            }
            return Err(format!("{} not found in downloaded archive", bin_name));
        }

        // On Unix, rename old binary first (atomic-ish replacement)
        #[cfg(unix)]
        {
            let backup = dest.with_extension("old");
            if dest.exists() {
                std::fs::rename(&dest, &backup)
                    .map_err(|e| format!("Failed to backup {}: {}", bin_name, e))?;
            }
            match std::fs::rename(&src, &dest) {
                Ok(()) => {
                    let _ = std::fs::remove_file(&backup);
                }
                Err(e) => {
                    // Try copy instead (cross-device rename)
                    if let Err(copy_err) = std::fs::copy(&src, &dest) {
                        // Restore backup
                        if backup.exists() {
                            let _ = std::fs::rename(&backup, &dest);
                        }
                        return Err(format!(
                            "Failed to install {}: rename={}, copy={}",
                            bin_name, e, copy_err
                        ));
                    }
                    let _ = std::fs::remove_file(&backup);
                }
            }
            // Ensure executable permission
            let _ = Command::new("chmod").arg("+x").arg(&dest).status();
        }

        #[cfg(windows)]
        {
            // On Windows, rename current exe out of the way first
            let backup = dest.with_extension("old.exe");
            if dest.exists() {
                std::fs::rename(&dest, &backup)
                    .map_err(|e| format!("Failed to backup {}: {}", bin_name, e))?;
            }
            match std::fs::rename(&src, &dest) {
                Ok(()) => {
                    let _ = std::fs::remove_file(&backup);
                }
                Err(e) => {
                    if backup.exists() {
                        let _ = std::fs::rename(&backup, &dest);
                    }
                    return Err(format!("Failed to install {}: {}", bin_name, e));
                }
            }
        }
    }

    Ok(())
}

/// Manual upgrade command — downloads and installs the latest version.
pub fn upgrade() {
    println!("git-worktree-manager v{}", CURRENT_VERSION);

    let info = match fetch_latest_release() {
        Some(info) => info,
        None => {
            println!(
                "{}",
                style("Could not check for updates. Check your internet connection.").red()
            );
            return;
        }
    };

    if !is_newer(&info.version, CURRENT_VERSION) {
        println!("{}", style("Already up to date.").green());
        return;
    }

    println!(
        "New version available: {} → {}",
        style(format!("v{}", CURRENT_VERSION)).dim(),
        style(format!("v{}", info.version)).green().bold()
    );

    // If running in a non-interactive context, just print the info
    if !std::io::stdin().is_terminal() {
        println!(
            "Download from: https://github.com/{}/releases/tag/{}",
            GITHUB_REPO, info.tag_name
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

    let target = match current_target() {
        Some(t) => t,
        None => {
            println!(
                "{}",
                style("Unsupported platform for auto-upgrade.").red()
            );
            println!(
                "Download manually: https://github.com/{}/releases/tag/{}",
                GITHUB_REPO, info.tag_name
            );
            return;
        }
    };

    match download_and_extract(&info.tag_name, target) {
        Ok(tmp_dir) => match self_replace(&tmp_dir) {
            Ok(()) => {
                let _ = std::fs::remove_dir_all(&tmp_dir);
                println!(
                    "{}",
                    style(format!("Upgraded to v{}!", info.version))
                        .green()
                        .bold()
                );
            }
            Err(e) => {
                let _ = std::fs::remove_dir_all(&tmp_dir);
                println!("{}", style(format!("Upgrade failed: {}", e)).red());
                println!(
                    "Download manually: https://github.com/{}/releases/tag/{}",
                    GITHUB_REPO, info.tag_name
                );
            }
        },
        Err(e) => {
            println!("{}", style(format!("Download failed: {}", e)).red());
            println!(
                "Download manually: https://github.com/{}/releases/tag/{}",
                GITHUB_REPO, info.tag_name
            );
        }
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
    fn test_current_target() {
        // Should return Some on supported platforms
        let target = current_target();
        assert!(target.is_some(), "current platform should be supported");
        let t = target.unwrap();
        assert!(t.contains("linux") || t.contains("darwin") || t.contains("windows"));
    }

    #[test]
    fn test_version_extraction_from_tag() {
        // Simulate the tag parsing logic
        let extract = |tag: &str| -> String {
            tag.strip_prefix("git-worktree-manager-v")
                .or_else(|| tag.strip_prefix('v'))
                .unwrap_or(tag)
                .to_string()
        };
        assert_eq!(extract("git-worktree-manager-v0.0.2"), "0.0.2");
        assert_eq!(extract("v0.0.1"), "0.0.1");
        assert_eq!(extract("0.0.3"), "0.0.3");
    }
}
