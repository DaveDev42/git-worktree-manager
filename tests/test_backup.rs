/// Tests for backup and restore operations.
/// Ported from tests/test_backup_restore.py (15 tests).
mod common;

use common::TestRepo;

#[test]
fn test_backup_list_runs() {
    let repo = TestRepo::new();
    let output = repo.cw(&["backup", "list"]);
    assert!(
        output.status.success(),
        "backup list failed: status={:?}\n--- stdout ---\n{}\n--- stderr ---\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    // May show "No backups" or existing backups from other tests
}

#[test]
fn test_backup_create_current() {
    let repo = TestRepo::new();
    // Create a worktree first
    let output = repo.cw(&["new", "backup-test", "--no-term"]);
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
    repo.cw(&["new", "list-test", "--no-term"]);
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
    repo.cw(&["new", "dirty-test", "--no-term"]);

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
    repo.cw(&["new", "filter-a", "--no-term"]);
    repo.cw(&["new", "filter-b", "--no-term"]);
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
    assert!(
        output.status.success(),
        "backup --help failed: status={:?}\n--- stdout ---\n{}\n--- stderr ---\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("create"));
    assert!(stdout.contains("list"));
    assert!(stdout.contains("restore"));
}

/// Regression: `backup create` on the main worktree (no per-branch config
/// recording a base_path) previously wrote metadata with `base_path: null`,
/// which made `backup list` filter the entry out of the current-repo view.
#[test]
fn test_backup_list_includes_main_worktree_backup() {
    let repo = TestRepo::new();

    let output = repo.cw(&["backup", "create"]);
    assert!(
        output.status.success(),
        "backup create failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = repo.cw_stdout(&["backup", "list"]);
    assert!(
        stdout.contains("main:"),
        "expected 'main:' header in backup list, got:\n{}",
        stdout
    );
}

/// Regression: a branch name containing '/' (e.g. `feat/A`) previously
/// created a nested `<backups>/feat/A/` layout, which broke
/// `list_backups`'s per-branch `read_dir` iteration. The on-disk layout
/// must be flat (`feat-A/`) while the display still shows the original
/// `feat/A` taken from metadata.
#[test]
fn test_backup_with_slash_in_branch() {
    let repo = TestRepo::new();
    // The TestRepo::create_worktree helper assumes the default flattened
    // path and asserts the directory exists; invoke `gw new` directly so
    // the slash in the branch name is handled end-to-end by gw itself.
    let output = repo.cw(&["new", "feat/A", "--no-term"]);
    assert!(
        output.status.success(),
        "gw new feat/A failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let output = repo.cw(&["backup", "create", "feat/A"]);
    assert!(
        output.status.success(),
        "backup create failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let backups_dir = git_worktree_manager::config::get_config_path()
        .parent()
        .expect("config path has parent")
        .join("backups");
    assert!(
        backups_dir.join("feat-A").is_dir(),
        "expected flattened 'feat-A' dir under {}",
        backups_dir.display()
    );
    assert!(
        !backups_dir.join("feat").join("A").exists(),
        "nested 'feat/A' dir should not exist"
    );

    let stdout = repo.cw_stdout(&["backup", "list"]);
    assert!(
        stdout.contains("feat/A:"),
        "backup list should show the original 'feat/A:' heading, got:\n{}",
        stdout
    );
}

/// `backup create --output <dir>` overrides the backups root, writing the
/// bundle and metadata under the user-specified directory instead of the
/// default `~/.config/.../backups`.
#[test]
fn test_backup_create_with_output_dir() {
    let repo = TestRepo::new();
    repo.cw(&["new", "out-test", "--no-term"]);

    let custom_dir = repo.path().join("custom-backups");
    let custom_str = custom_dir.to_string_lossy().to_string();

    let output = repo.cw(&["backup", "create", "out-test", "--output", &custom_str]);
    assert!(
        output.status.success(),
        "backup create --output failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(
        custom_dir.join("out-test").is_dir(),
        "expected backup under custom output dir at {}",
        custom_dir.display()
    );

    // Default backups dir should NOT contain this branch's backup.
    let default_dir = git_worktree_manager::config::get_config_path()
        .parent()
        .expect("config path has parent")
        .join("backups");
    assert!(
        !default_dir.join("out-test").exists(),
        "backup must not appear in default dir when --output is given",
    );
}
