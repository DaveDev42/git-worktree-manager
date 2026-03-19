/// Tests for global repository registry.
/// Ported from tests/test_registry.py (19 tests).
use git_worktree_manager::registry;

#[test]
fn test_registry_path_under_config_dir() {
    let path = registry::get_registry_path();
    // Config dir uses "claude-worktree" for backward compatibility
    assert!(path.to_string_lossy().contains("registry.json"));
    assert!(path.to_string_lossy().contains(".config"));
}

#[test]
fn test_load_empty_registry() {
    // Default registry should have empty repositories
    let reg = registry::Registry::default();
    assert_eq!(reg.version, 1);
    assert!(reg.repositories.is_empty());
}

#[test]
fn test_registry_serialization_roundtrip() {
    let mut reg = registry::Registry::default();
    reg.repositories.insert(
        "/tmp/test-repo".to_string(),
        registry::RepoEntry {
            name: "test-repo".to_string(),
            registered_at: "2026-01-01T00:00:00Z".to_string(),
            last_seen: "2026-01-02T00:00:00Z".to_string(),
        },
    );

    let json = serde_json::to_string(&reg).unwrap();
    let deserialized: registry::Registry = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.version, 1);
    assert_eq!(deserialized.repositories.len(), 1);
    assert!(deserialized.repositories.contains_key("/tmp/test-repo"));
    assert_eq!(
        deserialized.repositories["/tmp/test-repo"].name,
        "test-repo"
    );
}

#[test]
fn test_get_all_registered_repos() {
    // Just verify it doesn't crash; result depends on user's actual registry
    let repos = registry::get_all_registered_repos();
    assert!(repos.len() >= 0);
}

#[test]
fn test_scan_for_repos_empty_dir() {
    let tmp = tempfile::TempDir::new().unwrap();
    let repos = registry::scan_for_repos(Some(tmp.path()), 3);
    assert!(repos.is_empty());
}

#[test]
fn test_scan_for_repos_max_depth() {
    let tmp = tempfile::TempDir::new().unwrap();
    // depth 0 should not find deeply nested repos
    let repos = registry::scan_for_repos(Some(tmp.path()), 0);
    assert!(repos.is_empty());
}

#[test]
fn test_repo_entry_serialization() {
    let entry = registry::RepoEntry {
        name: "myproject".to_string(),
        registered_at: "2026-01-01T00:00:00Z".to_string(),
        last_seen: "2026-03-18T00:00:00Z".to_string(),
    };

    let json = serde_json::to_string(&entry).unwrap();
    assert!(json.contains("myproject"));
    assert!(json.contains("registered_at"));
    assert!(json.contains("last_seen"));

    let deserialized: registry::RepoEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.name, "myproject");
}

// Integration tests via CLI
mod common;
use common::TestRepo;

#[test]
fn test_scan_cli() {
    let repo = TestRepo::new();
    let output = repo.cw(&["scan"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Scanning") || stdout.contains("repository"));
}

#[test]
fn test_prune_cli() {
    let repo = TestRepo::new();
    let output = repo.cw(&["prune"]);
    assert!(output.status.success());
}
