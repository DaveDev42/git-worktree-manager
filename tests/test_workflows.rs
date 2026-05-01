/// End-to-end workflow tests.
/// Ported from tests/e2e/test_workflows.py.
mod common;

use common::TestRepo;

#[test]
fn test_full_workflow_new_list_delete() {
    let repo = TestRepo::new();

    // Create
    assert!(repo.cw_ok(&["new", "e2e-test", "--no-term"]));

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
fn test_workflow_multiple_worktrees() {
    let repo = TestRepo::new();
    assert!(repo.cw_ok(&["new", "feat-a", "--no-term"]));
    assert!(repo.cw_ok(&["new", "feat-b", "--no-term"]));
    assert!(repo.cw_ok(&["new", "feat-c", "--no-term"]));

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
    assert!(repo.cw_ok(&["new", "keep-branch-test", "--no-term"]));
    assert!(repo.cw_ok(&["delete", "keep-branch-test", "--keep-branch"]));

    // Worktree removed but branch should still exist
    let list = repo.cw_stdout(&["list"]);
    assert!(!list.contains("keep-branch-test"));
}

#[test]
fn test_workflow_doctor_after_operations() {
    let repo = TestRepo::new();
    assert!(repo.cw_ok(&["new", "doc-test", "--no-term"]));
    let doctor = repo.cw_stdout(&["doctor"]);
    assert!(doctor.contains("Health Check"));
    assert!(doctor.contains("Git version"));
}

#[test]
fn test_workflow_path_list_branches() {
    let repo = TestRepo::new();
    assert!(repo.cw_ok(&["new", "path-test", "--no-term"]));

    let output = repo.cw_stdout(&["_path", "--list-branches"]);
    assert!(output.contains("main") || output.contains("path-test"));
}

#[test]
fn test_workflow_backup_create_list() {
    let repo = TestRepo::new();
    assert!(repo.cw_ok(&["new", "backup-wf", "--no-term"]));

    let output = repo.cw(&["backup", "create", "backup-wf"]);
    assert!(output.status.success());

    let list = repo.cw_stdout(&["backup", "list"]);
    assert!(
        list.contains("backup-wf") || list.contains("Backup"),
        "Should list the backup"
    );
}

#[test]
fn test_workflow_new_with_prompt_file() {
    use std::io::Write;
    let repo = TestRepo::new();

    let mut prompt_file = tempfile::NamedTempFile::new().unwrap();
    writeln!(prompt_file, "do the thing").unwrap();

    let ok = repo.cw_ok(&[
        "new",
        "prompt-file-test",
        "--no-term",
        "--prompt-file",
        prompt_file.path().to_str().unwrap(),
    ]);
    assert!(ok, "gw new --prompt-file should succeed");

    let list = repo.cw_stdout(&["list"]);
    assert!(list.contains("prompt-file-test"));
}

#[test]
fn test_workflow_new_with_prompt_stdin() {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let repo = TestRepo::new();

    let mut child = Command::new(TestRepo::cw_bin())
        .args(["new", "prompt-stdin-test", "--no-term", "--prompt-stdin"])
        .current_dir(repo.path())
        .env("CW_LAUNCH_METHOD", "foreground")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn gw");

    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"piped task description\n")
        .unwrap();
    // Close the write end so the child sees EOF; also avoids any potential
    // deadlock if wait_with_output() ever reads a full pipe buffer.
    drop(child.stdin.take());
    let output = child.wait_with_output().expect("wait");
    assert!(
        output.status.success(),
        "gw new --prompt-stdin failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let list = repo.cw_stdout(&["list"]);
    assert!(list.contains("prompt-stdin-test"));
}

#[test]
fn test_workflow_new_prompt_file_missing_fails_cleanly() {
    let repo = TestRepo::new();

    let ok = repo.cw_ok(&[
        "new",
        "no-worktree-created",
        "--no-term",
        "--prompt-file",
        "/nonexistent/does/not/exist.txt",
    ]);
    assert!(!ok, "missing prompt file should fail the command");

    let list = repo.cw_stdout(&["list"]);
    assert!(
        !list.contains("no-worktree-created"),
        "worktree should NOT be created when prompt file is missing"
    );
}
