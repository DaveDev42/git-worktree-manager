/// Auto-update check and self-upgrade via GitHub Releases.
///
use std::io::{IsTerminal, Read};
use std::path::PathBuf;
use std::process::Command;

use console::style;
use serde::{Deserialize, Serialize};

use crate::constants::home_dir_or_fallback;

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const REPO_OWNER: &str = "DaveDev42";
const REPO_NAME: &str = "git-worktree-manager";

/// How often to check for updates (in seconds). Default: 6 hours.
const CHECK_INTERVAL_SECS: u64 = 6 * 60 * 60;

/// Cache for update check results.
#[derive(Debug, Serialize, Deserialize, Default)]
struct UpdateCache {
    /// Unix timestamp of last check.
    #[serde(default)]
    last_check_ts: u64,
    /// Legacy date string (for backward compat).
    #[serde(default)]
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

fn now_ts() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn cache_is_fresh(cache: &UpdateCache) -> bool {
    let age = now_ts().saturating_sub(cache.last_check_ts);
    age < CHECK_INTERVAL_SECS
}

/// Check for updates (called on startup).
///
/// Phase 1 (instant, no I/O): show notification from cache if newer version known.
/// Phase 2 (background): if cache is stale, fork a background process to refresh it.
pub fn check_for_update_if_needed() {
    let config = crate::config::load_config().unwrap_or_default();
    if !config.update.auto_check {
        return;
    }

    let cache = load_cache();

    // Phase 1: instant notification from cache (zero latency)
    if let Some(ref latest) = cache.latest_version {
        if is_newer(latest, CURRENT_VERSION) {
            eprintln!(
                "\n{} {} is available (current: {})",
                style("git-worktree-manager").bold(),
                style(format!("v{}", latest)).green(),
                style(format!("v{}", CURRENT_VERSION)).dim(),
            );
            eprintln!("Run '{}' to update.\n", style("gw upgrade").cyan().bold());
        }
    }

    // Phase 2: if cache is stale, refresh in background
    if !cache_is_fresh(&cache) {
        spawn_background_check();
    }
}

/// Spawn a background process to check for updates without blocking startup.
fn spawn_background_check() {
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return,
    };
    // Use a hidden subcommand to do the actual check
    let _ = Command::new(exe)
        .arg("_update-cache")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

/// Refresh the update cache (called by background process).
pub fn refresh_cache() {
    if let Some(latest) = fetch_latest_version() {
        let cache = UpdateCache {
            last_check_ts: now_ts(),
            latest_version: Some(latest),
            ..Default::default()
        };
        save_cache(&cache);
    } else {
        // Save timestamp even on failure to avoid hammering
        let cache = UpdateCache {
            last_check_ts: now_ts(),
            latest_version: load_cache().latest_version, // keep previous
            ..Default::default()
        };
        save_cache(&cache);
    }
}

/// Get a GitHub auth token if available.
/// Checks GITHUB_TOKEN env var first, then falls back to `gh auth token`.
fn gh_auth_token() -> Option<String> {
    // 1. Environment variable (fast, no subprocess)
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        if !token.is_empty() {
            return Some(token);
        }
    }
    if let Ok(token) = std::env::var("GH_TOKEN") {
        if !token.is_empty() {
            return Some(token);
        }
    }

    // 2. gh CLI (only if binary exists)
    if which_exists("gh") {
        return Command::new("gh")
            .args(["auth", "token"])
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|t| !t.is_empty());
    }

    None
}

/// Check if a command exists in PATH without running it.
fn which_exists(cmd: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths).any(|dir| {
                let full = dir.join(cmd);
                full.is_file() || (cfg!(windows) && dir.join(format!("{}.exe", cmd)).is_file())
            })
        })
        .unwrap_or(false)
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
        "--max-time".to_string(),
        "10".to_string(),
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
    let real_path = match std::fs::canonicalize(&exe) {
        Ok(p) => p,
        Err(_) => exe,
    };
    let path_str = real_path.to_string_lossy();
    path_str.contains("/Cellar/") || path_str.contains("/homebrew/")
}

/// Determine the current platform target triple.
fn current_target() -> &'static str {
    #[cfg(all(target_arch = "x86_64", target_os = "macos"))]
    {
        "x86_64-apple-darwin"
    }
    #[cfg(all(target_arch = "aarch64", target_os = "macos"))]
    {
        "aarch64-apple-darwin"
    }
    #[cfg(all(target_arch = "x86_64", target_os = "windows"))]
    {
        "x86_64-pc-windows-msvc"
    }
    #[cfg(all(target_arch = "x86_64", target_os = "linux"))]
    {
        "x86_64-unknown-linux-musl"
    }
    #[cfg(all(target_arch = "aarch64", target_os = "linux"))]
    {
        "aarch64-unknown-linux-musl"
    }
}

/// Archive extension for the current platform.
fn archive_ext() -> &'static str {
    if cfg!(windows) {
        "zip"
    } else {
        "tar.gz"
    }
}

/// Download the release asset and extract the binary to a temp file.
/// Returns the path to the extracted binary.
fn download_and_extract(version: &str) -> Result<PathBuf, String> {
    let target = current_target();
    let asset_name = format!("gw-{}.{}", target, archive_ext());
    let url = format!(
        "https://github.com/{}/{}/releases/download/v{}/{}",
        REPO_OWNER, REPO_NAME, version, asset_name
    );

    // Build HTTP client (uses system CA store via rustls-native-roots)
    let client = reqwest::blocking::Client::builder()
        .user_agent(format!("gw/{}", CURRENT_VERSION))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let response = client
        .get(&url)
        .send()
        .map_err(|e| format!("Download failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "Download failed: HTTP {} for {}",
            response.status(),
            asset_name
        ));
    }

    let total_size = response.content_length().unwrap_or(0);

    // Set up progress bar
    let pb = if total_size > 0 {
        let pb = indicatif::ProgressBar::new(total_size);
        pb.set_style(
            indicatif::ProgressStyle::default_bar()
                .template("{bar:40.cyan/blue} {bytes}/{total_bytes} ({eta})")
                .unwrap_or_else(|_| indicatif::ProgressStyle::default_bar())
                .progress_chars("█▓░"),
        );
        pb
    } else {
        let pb = indicatif::ProgressBar::new_spinner();
        pb.set_style(
            indicatif::ProgressStyle::default_spinner()
                .template("{spinner} {bytes} downloaded")
                .unwrap_or_else(|_| indicatif::ProgressStyle::default_spinner()),
        );
        pb
    };

    // Download to memory with progress
    let mut downloaded: Vec<u8> = Vec::with_capacity(total_size as usize);
    let mut reader = response;
    let mut buf = [0u8; 8192];
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| format!("Read error: {}", e))?;
        if n == 0 {
            break;
        }
        downloaded.extend_from_slice(&buf[..n]);
        pb.set_position(downloaded.len() as u64);
    }
    pb.finish_and_clear();

    // Extract binary
    let tmp_dir = tempfile::tempdir().map_err(|e| format!("Failed to create temp dir: {}", e))?;
    let bin_name = if cfg!(windows) { "gw.exe" } else { "gw" };

    if cfg!(windows) {
        extract_zip(&downloaded, tmp_dir.path(), bin_name)?;
    } else {
        extract_tar_gz(&downloaded, tmp_dir.path(), bin_name)?;
    }

    let extracted_bin = tmp_dir.path().join(bin_name);
    if !extracted_bin.exists() {
        return Err(format!(
            "Binary '{}' not found in archive. Contents may have unexpected layout.",
            bin_name
        ));
    }

    // Move to a persistent temp file (tempdir would delete on drop)
    let persistent_path = std::env::temp_dir().join(format!("gw-update-{}", version));
    std::fs::copy(&extracted_bin, &persistent_path)
        .map_err(|e| format!("Failed to copy binary: {}", e))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&persistent_path, std::fs::Permissions::from_mode(0o755));
    }

    Ok(persistent_path)
}

/// Extract a tar.gz archive and find the binary.
#[cfg(not(windows))]
fn extract_tar_gz(data: &[u8], dest: &std::path::Path, bin_name: &str) -> Result<(), String> {
    let gz = flate2::read::GzDecoder::new(data);
    let mut archive = tar::Archive::new(gz);

    for entry in archive.entries().map_err(|e| format!("tar error: {}", e))? {
        let mut entry = entry.map_err(|e| format!("tar entry error: {}", e))?;
        let path = entry
            .path()
            .map_err(|e| format!("tar path error: {}", e))?
            .into_owned();

        // Extract only the binary (may be at root or in a subdirectory)
        if path.file_name().and_then(|n| n.to_str()) == Some(bin_name) {
            let out_path = dest.join(bin_name);
            let mut out_file = std::fs::File::create(&out_path)
                .map_err(|e| format!("Failed to create file: {}", e))?;
            std::io::copy(&mut entry, &mut out_file)
                .map_err(|e| format!("Failed to write file: {}", e))?;
            return Ok(());
        }
    }
    Err(format!("'{}' not found in tar.gz archive", bin_name))
}

/// Extract a zip archive and find the binary.
#[cfg(windows)]
fn extract_zip(data: &[u8], dest: &std::path::Path, bin_name: &str) -> Result<(), String> {
    let cursor = std::io::Cursor::new(data);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|e| format!("zip error: {}", e))?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("zip entry error: {}", e))?;
        let name = file.name().to_string();

        if name.ends_with(bin_name)
            || std::path::Path::new(&name)
                .file_name()
                .and_then(|n| n.to_str())
                == Some(bin_name)
        {
            let out_path = dest.join(bin_name);
            let mut out_file = std::fs::File::create(&out_path)
                .map_err(|e| format!("Failed to create file: {}", e))?;
            std::io::copy(&mut file, &mut out_file)
                .map_err(|e| format!("Failed to write file: {}", e))?;
            return Ok(());
        }
    }
    Err(format!("'{}' not found in zip archive", bin_name))
}

// Provide stub functions for platforms where the other archive format isn't used,
// so the code compiles on all targets (they're never called).
#[cfg(windows)]
fn extract_tar_gz(_data: &[u8], _dest: &std::path::Path, _bin_name: &str) -> Result<(), String> {
    Err("tar.gz extraction not supported on Windows".to_string())
}

#[cfg(not(windows))]
fn extract_zip(_data: &[u8], _dest: &std::path::Path, _bin_name: &str) -> Result<(), String> {
    Err("zip extraction not used on Unix".to_string())
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

    // Update cache with fresh data
    let cache = UpdateCache {
        last_check_ts: now_ts(),
        latest_version: Some(latest_version.clone()),
        ..Default::default()
    };
    save_cache(&cache);

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

    println!("Downloading v{}...", latest_version);

    match download_and_extract(&latest_version) {
        Ok(new_binary) => {
            // Replace the running binary
            if let Err(e) = self_replace::self_replace(&new_binary) {
                println!(
                    "{}",
                    style(format!("Failed to replace binary: {}", e)).red()
                );
                println!(
                    "Download manually: https://github.com/{}/{}/releases/latest",
                    REPO_OWNER, REPO_NAME
                );
                let _ = std::fs::remove_file(&new_binary);
                return;
            }

            // Clean up temp file
            let _ = std::fs::remove_file(&new_binary);

            update_companion_binary();
            println!(
                "{}",
                style(format!("Upgraded to v{}!", latest_version))
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
        assert!(!is_homebrew_install());
    }

    #[test]
    fn test_cache_freshness() {
        let fresh = UpdateCache {
            last_check_ts: now_ts(),
            latest_version: Some("1.0.0".into()),
            ..Default::default()
        };
        assert!(cache_is_fresh(&fresh));

        let stale = UpdateCache {
            last_check_ts: now_ts() - CHECK_INTERVAL_SECS - 1,
            latest_version: Some("1.0.0".into()),
            ..Default::default()
        };
        assert!(!cache_is_fresh(&stale));

        let empty = UpdateCache::default();
        assert!(!cache_is_fresh(&empty));
    }

    #[test]
    fn test_current_target() {
        let target = current_target();
        assert!(!target.is_empty());
        // Should match one of our supported targets
        let valid = [
            "x86_64-apple-darwin",
            "aarch64-apple-darwin",
            "x86_64-pc-windows-msvc",
            "x86_64-unknown-linux-musl",
            "aarch64-unknown-linux-musl",
        ];
        assert!(valid.contains(&target));
    }

    #[test]
    fn test_archive_ext() {
        let ext = archive_ext();
        if cfg!(windows) {
            assert_eq!(ext, "zip");
        } else {
            assert_eq!(ext, "tar.gz");
        }
    }
}
