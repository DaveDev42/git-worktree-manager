//! File-write helpers used by the marketplace install. Writes are
//! content-addressed (skip when bytes match) so re-running `gw
//! setup-claude` is a true no-op when nothing changed.

use std::io;
use std::path::Path;

/// Write `content` to `path`. If the file already exists with byte-identical
/// content, do nothing. Returns `true` if a write actually occurred.
/// Creates any missing parent directories.
pub fn write_if_changed(path: &Path, content: &str) -> io::Result<bool> {
    if path.exists() {
        let existing = std::fs::read_to_string(path).unwrap_or_default();
        if existing == content {
            return Ok(false);
        }
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)?;
    Ok(true)
}

/// Drop a sentinel marker that lets future `gw setup-claude` runs (and
/// the legacy-cleanup step) recognize a directory we own.
pub fn write_sentinel(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
        path,
        "managed by git-worktree-manager (`gw setup-claude`); safe to delete on uninstall\n",
    )
}

pub fn sentinel_present(path: &Path) -> bool {
    path.exists()
}
