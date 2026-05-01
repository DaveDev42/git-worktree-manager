//! Tests for the `.cwconfig.json` walk-up loader.

use git_worktree_manager::repo_config::find_repo_config;
use tempfile::tempdir;

#[test]
fn finds_cwconfig_walking_up_from_subdir() {
    let dir = tempdir().unwrap();
    std::fs::write(
        dir.path().join(".cwconfig.json"),
        r#"{"ai_tool":{"command":"codex","args":[]}}"#,
    )
    .unwrap();
    let nested = dir.path().join("a/b/c");
    std::fs::create_dir_all(&nested).unwrap();

    let found = find_repo_config(&nested).expect("found");
    // Canonicalize both sides because tempdir on macOS lives under
    // /var/folders/... which is a symlink to /private/var/folders/...
    let lhs = found.path.canonicalize().unwrap_or(found.path.clone());
    let rhs = dir
        .path()
        .join(".cwconfig.json")
        .canonicalize()
        .unwrap_or_else(|_| dir.path().join(".cwconfig.json"));
    assert_eq!(lhs, rhs);
    assert_eq!(found.value["ai_tool"]["command"], "codex");
}

#[test]
fn returns_none_when_absent() {
    let dir = tempdir().unwrap();
    let found = find_repo_config(dir.path());
    assert!(found.is_none());
}

#[test]
fn invalid_json_yields_none() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join(".cwconfig.json"), b"{ not json").unwrap();
    let found = find_repo_config(dir.path());
    assert!(
        found.is_none(),
        "broken JSON should fail closed (return None)"
    );
}
