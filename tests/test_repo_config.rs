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

// ---------------------------------------------------------------------------
// Layered config resolution tests (Phase 7.2)
// ---------------------------------------------------------------------------

#[test]
fn repo_config_overrides_global_keys() {
    use std::path::PathBuf;
    let global_dir = tempdir().unwrap();
    let global_path: PathBuf = global_dir.path().join("config.json");
    std::fs::write(
        &global_path,
        r#"{"ai_tool":{"command":"claude","args":[]}}"#,
    )
    .unwrap();

    let repo = tempdir().unwrap();
    std::fs::write(
        repo.path().join(".cwconfig.json"),
        r#"{"ai_tool":{"command":"codex","args":[]}}"#,
    )
    .unwrap();

    let cfg =
        git_worktree_manager::config::load_effective_config_with_global(repo.path(), &global_path)
            .unwrap();
    assert_eq!(cfg.ai_tool.command, "codex");
}

#[test]
fn repo_config_extends_global_when_no_overlap() {
    let global_dir = tempdir().unwrap();
    let global_path = global_dir.path().join("config.json");
    std::fs::write(
        &global_path,
        r#"{"ai_tool":{"command":"claude","args":["--verbose"]}}"#,
    )
    .unwrap();

    let repo = tempdir().unwrap();
    // No .cwconfig.json — global is the only override.
    let cfg =
        git_worktree_manager::config::load_effective_config_with_global(repo.path(), &global_path)
            .unwrap();
    assert_eq!(cfg.ai_tool.command, "claude");
    assert_eq!(cfg.ai_tool.args, vec!["--verbose"]);
}

#[test]
fn missing_global_and_repo_returns_defaults() {
    let global_dir = tempdir().unwrap();
    let global_path = global_dir.path().join("nonexistent.json"); // does not exist
    let repo = tempdir().unwrap();
    let cfg =
        git_worktree_manager::config::load_effective_config_with_global(repo.path(), &global_path)
            .unwrap();
    let default = git_worktree_manager::config::Config::default();
    assert_eq!(cfg.ai_tool.command, default.ai_tool.command);
}

/// Confirms `deep_merge` semantics flow through `load_effective_config_with_global`:
/// repo overrides `ai_tool.command` while keeping the global `ai_tool.args`.
#[test]
fn nested_keys_merge_independently() {
    let global_dir = tempdir().unwrap();
    let global_path = global_dir.path().join("config.json");
    std::fs::write(
        &global_path,
        r#"{"ai_tool":{"command":"claude","args":["--verbose","--debug"]}}"#,
    )
    .unwrap();

    let repo = tempdir().unwrap();
    std::fs::write(
        repo.path().join(".cwconfig.json"),
        r#"{"ai_tool":{"command":"codex"}}"#,
    )
    .unwrap();

    let cfg =
        git_worktree_manager::config::load_effective_config_with_global(repo.path(), &global_path)
            .unwrap();
    assert_eq!(cfg.ai_tool.command, "codex", "repo overrides command");
    assert_eq!(
        cfg.ai_tool.args,
        vec!["--verbose", "--debug"],
        "global args survive when repo doesn't override them"
    );
}
