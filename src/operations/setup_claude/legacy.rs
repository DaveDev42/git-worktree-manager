//! Removal of pre-marketplace install locations.
//!
//! Three layouts have shipped and need cleanup on `gw setup-claude` re-run:
//! 1. `~/.claude/skills/gw/`           — original single-skill install
//! 2. `~/.claude/skills/gw-delegate/`  — second single-skill iteration
//! 3. `~/.claude/plugins/gw/`          — broken plugin layout that Claude
//!    Code never recognized (manifest at the wrong path, no marketplace
//!    registration). This is the layout we are replacing in this change.
//!
//! All three are safe to remove because they were exclusively created by
//! `gw setup-claude` — we know nothing else writes to those exact paths.

use std::path::{Path, PathBuf};

use crate::constants::home_dir_or_fallback;

fn legacy_paths_under(home: &Path) -> Vec<PathBuf> {
    let claude = home.join(".claude");
    vec![
        claude.join("skills").join("gw"),
        claude.join("skills").join("gw-delegate"),
        claude.join("plugins").join("gw"),
    ]
}

pub fn any_legacy_present() -> bool {
    any_legacy_present_under(&home_dir_or_fallback())
}

pub fn any_legacy_present_under(home: &Path) -> bool {
    legacy_paths_under(home).iter().any(|p| p.exists())
}

pub fn remove_legacy_installs_under(home: &Path) {
    for p in legacy_paths_under(home) {
        if p.exists() {
            let _ = std::fs::remove_dir_all(&p);
        }
    }
}
