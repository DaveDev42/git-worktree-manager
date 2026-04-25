//! Removal of pre-plugin install locations.

use std::path::PathBuf;

use crate::constants::home_dir_or_fallback;

fn legacy_paths() -> Vec<PathBuf> {
    let base = home_dir_or_fallback().join(".claude").join("skills");
    vec![base.join("gw"), base.join("gw-delegate")]
}

pub fn any_legacy_present() -> bool {
    legacy_paths().iter().any(|p| p.exists())
}

pub fn remove_legacy_installs() {
    for p in legacy_paths() {
        if p.exists() {
            let _ = std::fs::remove_dir_all(&p);
        }
    }
}
