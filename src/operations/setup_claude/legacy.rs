//! Removal of pre-plugin install locations.

use std::path::{Path, PathBuf};

use crate::constants::home_dir_or_fallback;

fn legacy_paths_under(home: &Path) -> Vec<PathBuf> {
    let base = home.join(".claude").join("skills");
    vec![base.join("gw"), base.join("gw-delegate")]
}

pub fn any_legacy_present() -> bool {
    legacy_paths_under(&home_dir_or_fallback())
        .iter()
        .any(|p| p.exists())
}

pub fn remove_legacy_installs_under(home: &Path) {
    for p in legacy_paths_under(home) {
        if p.exists() {
            let _ = std::fs::remove_dir_all(&p);
        }
    }
}
