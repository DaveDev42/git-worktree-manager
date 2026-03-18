/// File sharing for worktrees via .cwshare configuration.
///
/// Reads .cwshare file from repository root and copies specified files
/// to new worktrees during creation.
use std::path::Path;

use console::style;

/// Parse .cwshare file and return list of paths to share.
pub fn parse_cwshare(repo_path: &Path) -> Vec<String> {
    let cwshare_path = repo_path.join(".cwshare");
    if !cwshare_path.exists() {
        return Vec::new();
    }

    match std::fs::read_to_string(&cwshare_path) {
        Ok(content) => content
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(String::from)
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Copy files specified in .cwshare to target worktree.
pub fn share_files(source_repo: &Path, target_worktree: &Path) {
    let paths = parse_cwshare(source_repo);
    if paths.is_empty() {
        return;
    }

    println!("\n{}", style("Copying shared files:").cyan().bold());

    for rel_path in &paths {
        let source = source_repo.join(rel_path);
        let target = target_worktree.join(rel_path);

        if !source.exists() || target.exists() {
            continue;
        }

        let result = if source.is_dir() {
            copy_dir_recursive(&source, &target)
        } else {
            if let Some(parent) = target.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            std::fs::copy(&source, &target).map(|_| ())
        };

        match result {
            Ok(()) => println!("  {} Copied: {}", style("*").green(), rel_path),
            Err(e) => println!("  {} Failed: {}: {}", style("!").yellow(), rel_path, e),
        }
    }
    println!();
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dst_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &dst_path)?;
        } else if ty.is_symlink() {
            let target = std::fs::read_link(entry.path())?;
            #[cfg(unix)]
            std::os::unix::fs::symlink(&target, &dst_path)?;
            #[cfg(windows)]
            std::os::windows::fs::symlink_file(&target, &dst_path)?;
        } else {
            std::fs::copy(entry.path(), &dst_path)?;
        }
    }
    Ok(())
}
