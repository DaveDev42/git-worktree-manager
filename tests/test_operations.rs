/// Integration tests for core operations using real git repos.
mod common;

use common::TestRepo;
use predicates::prelude::*;

#[test]
fn test_list_in_repo() {
    let repo = TestRepo::new();
    let output = repo.cw(&["list"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Worktrees for repository:"));
}

#[test]
fn test_status_in_repo() {
    let repo = TestRepo::new();
    let output = repo.cw(&["status"]);
    assert!(output.status.success());
}

#[test]
fn test_tree_in_repo() {
    let repo = TestRepo::new();
    let output = repo.cw(&["tree"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("(base repository)"));
}

#[test]
fn test_stats_no_worktrees() {
    let repo = TestRepo::new();
    let output = repo.cw(&["stats"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No feature worktrees found"));
}

#[test]
fn test_new_creates_worktree() {
    let repo = TestRepo::new();
    let output = repo.cw(&["new", "test-feature", "--no-ai"]);
    assert!(
        output.status.success(),
        "cw new failed: {}",
        String::from_utf8_lossy(&output.stdout)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Worktree created successfully"));

    // Verify worktree exists in list
    let list_output = repo.cw_stdout(&["list"]);
    assert!(list_output.contains("test-feature"));
}

#[test]
fn test_new_invalid_branch_name() {
    let repo = TestRepo::new();
    let output = repo.cw(&["new", "bad..name", "--no-ai"]);
    assert!(!output.status.success());
}

#[test]
fn test_new_then_delete() {
    let repo = TestRepo::new();

    // Create
    let output = repo.cw(&["new", "to-delete", "--no-ai"]);
    assert!(output.status.success());

    // Verify it exists
    let list = repo.cw_stdout(&["list"]);
    assert!(list.contains("to-delete"));

    // Delete
    let del_output = repo.cw(&["delete", "to-delete"]);
    assert!(
        del_output.status.success(),
        "delete failed: {}",
        String::from_utf8_lossy(&del_output.stdout)
    );

    // Verify it's gone
    let list_after = repo.cw_stdout(&["list"]);
    assert!(!list_after.contains("to-delete"));
}

#[test]
fn test_new_then_sync() {
    let repo = TestRepo::new();
    let output = repo.cw(&["new", "sync-test", "--no-ai"]);
    assert!(output.status.success());

    // Sync (should succeed even without remote)
    let sync_output = repo.cw(&["sync", "sync-test"]);
    // May warn about no remote, but shouldn't crash
    let stdout = String::from_utf8_lossy(&sync_output.stdout);
    assert!(stdout.contains("Fetching") || stdout.contains("Syncing") || stdout.contains("Sync"));
}

#[test]
fn test_doctor() {
    let repo = TestRepo::new();
    let output = repo.cw(&["doctor"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Health Check"));
    assert!(stdout.contains("Checking Git version"));
}

#[test]
fn test_config_show() {
    let repo = TestRepo::new();
    let output = repo.cw(&["config", "show"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("AI Tool:"));
    assert!(stdout.contains("Config file:"));
}

// backup, stash, hook tests moved to dedicated test files

#[test]
fn test_path_list_branches() {
    let repo = TestRepo::new();
    let output = repo.cw(&["_path", "--list-branches"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should have at least the main branch
    assert!(stdout.contains("main") || stdout.contains("master"));
}

#[test]
fn test_clean_no_criteria() {
    let repo = TestRepo::new();
    let output = repo.cw(&["clean"]);
    // Should complain about missing criteria
    assert!(output.status.success()); // exits 0 but prints error
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);
    assert!(combined.contains("criterion") || combined.contains("specify"));
}

#[test]
fn test_clean_dry_run() {
    let repo = TestRepo::new();
    let output = repo.cw(&["clean", "--merged", "--dry-run"]);
    assert!(output.status.success());
}

#[test]
fn test_diff_nonexistent_branch() {
    let repo = TestRepo::new();
    let output = repo.cw(&["diff", "main", "nonexistent"]);
    assert!(!output.status.success());
}

#[test]
fn test_prune_empty() {
    let repo = TestRepo::new();
    let output = repo.cw(&["prune"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("stale") || stdout.contains("No stale"));
}

#[test]
fn test_export_creates_file() {
    let repo = TestRepo::new();
    let export_path = repo.path().join("test-export.json");
    let output = repo.cw(&["export", "--output", export_path.to_str().unwrap()]);
    assert!(output.status.success());
    assert!(export_path.exists());

    // Verify it's valid JSON
    let content = std::fs::read_to_string(&export_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(
        parsed.get("export_version").unwrap().as_str().unwrap(),
        "1.0"
    );
}

#[test]
fn test_import_preview() {
    let repo = TestRepo::new();

    // Create export first
    let export_path = repo.path().join("import-test.json");
    repo.cw(&["export", "--output", export_path.to_str().unwrap()]);

    // Import in preview mode (no --apply)
    let output = repo.cw(&["import", export_path.to_str().unwrap()]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Preview"));
}
