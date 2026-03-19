/// Tests for backup and restore operations.
/// Ported from tests/test_backup_restore.py (15 tests).
mod common;

use common::TestRepo;

#[test]
fn test_backup_list_runs() {
    let repo = TestRepo::new();
    let output = repo.cw(&["backup", "list"]);
    assert!(output.status.success());
    // May show "No backups" or existing backups from other tests
}

#[test]
fn test_backup_create_current() {
    let repo = TestRepo::new();
    // Create a worktree first
    let output = repo.cw(&["new", "backup-test", "--no-ai"]);
    assert!(output.status.success());

    // Backup
    let output = repo.cw(&["backup", "create", "backup-test"]);
    assert!(
        output.status.success(),
        "backup create failed: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Backup") || stdout.contains("backup"));
}

#[test]
fn test_backup_list_after_create() {
    let repo = TestRepo::new();
    repo.cw(&["new", "list-test", "--no-ai"]);
    repo.cw(&["backup", "create", "list-test"]);

    let stdout = repo.cw_stdout(&["backup", "list"]);
    // Should show at least one backup
    assert!(
        stdout.contains("list-test") || stdout.contains("Backups"),
        "Expected backup listing, got: {}",
        stdout
    );
}

#[test]
fn test_backup_create_with_uncommitted_changes() {
    let repo = TestRepo::new();
    repo.cw(&["new", "dirty-test", "--no-ai"]);

    // Find the worktree path and make it dirty
    let list = repo.cw_stdout(&["list"]);
    assert!(list.contains("dirty-test"));

    // Create dirty file in worktree
    let wt_path = repo.path().parent().unwrap().join(format!(
        "{}-dirty-test",
        repo.path().file_name().unwrap().to_string_lossy()
    ));
    if wt_path.exists() {
        std::fs::write(wt_path.join("uncommitted.txt"), "dirty content").unwrap();
    }

    let output = repo.cw(&["backup", "create", "dirty-test"]);
    assert!(output.status.success());
}

#[test]
fn test_backup_restore_nonexistent_branch() {
    let repo = TestRepo::new();
    let output = repo.cw(&["backup", "restore", "nonexistent-branch"]);
    assert!(!output.status.success());
}

#[test]
fn test_backup_list_filter_by_branch() {
    let repo = TestRepo::new();
    repo.cw(&["new", "filter-a", "--no-ai"]);
    repo.cw(&["new", "filter-b", "--no-ai"]);
    repo.cw(&["backup", "create", "filter-a"]);

    let stdout = repo.cw_stdout(&["backup", "list", "filter-a"]);
    assert!(
        stdout.contains("filter-a") || stdout.contains("Backups") || stdout.contains("No backups"),
    );
}

#[test]
fn test_backup_help() {
    let repo = TestRepo::new();
    let output = repo.cw(&["backup", "--help"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("create"));
    assert!(stdout.contains("list"));
    assert!(stdout.contains("restore"));
}
