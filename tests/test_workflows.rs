/// End-to-end workflow tests.
/// Ported from tests/e2e/test_workflows.py (13 tests).
mod common;

use common::TestRepo;

#[test]
fn test_full_workflow_new_list_delete() {
    let repo = TestRepo::new();

    // Create
    assert!(repo.cw_ok(&["new", "e2e-test", "--no-ai"]));

    // List shows it
    let list = repo.cw_stdout(&["list"]);
    assert!(list.contains("e2e-test"));

    // Delete
    assert!(repo.cw_ok(&["delete", "e2e-test"]));

    // Gone
    let list = repo.cw_stdout(&["list"]);
    assert!(!list.contains("e2e-test"));
}

#[test]
fn test_workflow_new_status() {
    let repo = TestRepo::new();
    assert!(repo.cw_ok(&["new", "status-test", "--no-ai"]));

    let status = repo.cw_stdout(&["status"]);
    assert!(
        status.contains("Worktrees") || status.contains("worktree"),
        "status should show worktree info"
    );
}

#[test]
fn test_workflow_new_tree() {
    let repo = TestRepo::new();
    assert!(repo.cw_ok(&["new", "tree-test", "--no-ai"]));

    let tree = repo.cw_stdout(&["tree"]);
    assert!(tree.contains("tree-test"));
    assert!(tree.contains("base repository"));
}

#[test]
fn test_workflow_multiple_worktrees() {
    let repo = TestRepo::new();
    assert!(repo.cw_ok(&["new", "feat-a", "--no-ai"]));
    assert!(repo.cw_ok(&["new", "feat-b", "--no-ai"]));
    assert!(repo.cw_ok(&["new", "feat-c", "--no-ai"]));

    let list = repo.cw_stdout(&["list"]);
    assert!(list.contains("feat-a"));
    assert!(list.contains("feat-b"));
    assert!(list.contains("feat-c"));

    // Clean up one
    assert!(repo.cw_ok(&["delete", "feat-b"]));
    let list = repo.cw_stdout(&["list"]);
    assert!(!list.contains("feat-b"));
    assert!(list.contains("feat-a"));
    assert!(list.contains("feat-c"));
}

#[test]
fn test_workflow_delete_keep_branch() {
    let repo = TestRepo::new();
    assert!(repo.cw_ok(&["new", "keep-branch-test", "--no-ai"]));
    assert!(repo.cw_ok(&["delete", "keep-branch-test", "--keep-branch"]));

    // Worktree removed but branch should still exist
    let list = repo.cw_stdout(&["list"]);
    assert!(!list.contains("keep-branch-test"));
}

#[test]
fn test_workflow_doctor_after_operations() {
    let repo = TestRepo::new();
    assert!(repo.cw_ok(&["new", "doc-test", "--no-ai"]));
    let doctor = repo.cw_stdout(&["doctor"]);
    assert!(doctor.contains("Health Check"));
    assert!(doctor.contains("Git version"));
}

#[test]
fn test_workflow_config_show() {
    let repo = TestRepo::new();
    let output = repo.cw_stdout(&["config", "show"]);
    assert!(output.contains("AI Tool:"));
    assert!(output.contains("Config file:"));
}

#[test]
fn test_workflow_config_list_presets() {
    let repo = TestRepo::new();
    let output = repo.cw_stdout(&["config", "list-presets"]);
    assert!(output.contains("claude"));
    assert!(output.contains("no-op"));
}

#[test]
fn test_workflow_export_import_roundtrip() {
    let repo = TestRepo::new();
    let export_path = repo.path().join("export.json");

    assert!(repo.cw_ok(&["export", "--output", export_path.to_str().unwrap()]));
    assert!(export_path.exists());

    // Import preview
    let output = repo.cw(&["import", export_path.to_str().unwrap()]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Preview"));
}

#[test]
fn test_workflow_new_then_sync() {
    let repo = TestRepo::new();
    assert!(repo.cw_ok(&["new", "sync-wf", "--no-ai"]));

    let output = repo.cw(&["sync", "sync-wf"]);
    assert!(output.status.success());
}

#[test]
fn test_workflow_clean_dry_run() {
    let repo = TestRepo::new();
    assert!(repo.cw_ok(&["new", "clean-test", "--no-ai"]));

    let output = repo.cw(&["clean", "--merged", "--dry-run"]);
    assert!(output.status.success());
}

#[test]
fn test_workflow_path_list_branches() {
    let repo = TestRepo::new();
    assert!(repo.cw_ok(&["new", "path-test", "--no-ai"]));

    let output = repo.cw_stdout(&["_path", "--list-branches"]);
    assert!(output.contains("main") || output.contains("path-test"));
}

#[test]
fn test_workflow_backup_create_list() {
    let repo = TestRepo::new();
    assert!(repo.cw_ok(&["new", "backup-wf", "--no-ai"]));

    let output = repo.cw(&["backup", "create", "backup-wf"]);
    assert!(output.status.success());

    let list = repo.cw_stdout(&["backup", "list"]);
    assert!(
        list.contains("backup-wf") || list.contains("Backup"),
        "Should list the backup"
    );
}
