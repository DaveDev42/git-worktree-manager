//! Integration tests for `gw exec` — single-worktree dispatch.

mod common;
use common::TestRepo;

use git_worktree_manager::operations::exec::exec_in_target;

#[test]
fn exec_runs_cmd_in_named_worktree() {
    let repo = TestRepo::new();
    let _wt = repo.create_worktree("feat-x");

    let wt_path = repo.path().parent().unwrap().join(format!(
        "{}-feat-x",
        repo.path().file_name().unwrap().to_str().unwrap()
    ));

    let mut buf: Vec<u8> = Vec::new();
    let code = exec_in_target(
        &wt_path,
        // The strict resolver accepts the worktree-directory basename;
        // since the basename embeds a TempDir name, derive it explicitly.
        wt_path.file_name().unwrap().to_str().unwrap(),
        &["pwd".to_string()],
        &mut buf,
    )
    .expect("exec_in_target");

    assert_eq!(code, 0);
    let s = String::from_utf8(buf).expect("utf8");
    // Output prefixed with [<worktree-name>] (the basename).
    assert!(
        s.contains("-feat-x] "),
        "expected feat-x prefix in output; got: {s}"
    );
}

#[test]
fn exec_resolves_by_branch_name() {
    let repo = TestRepo::new();
    let _wt = repo.create_worktree("feat-x");

    let wt_path = repo.path().parent().unwrap().join(format!(
        "{}-feat-x",
        repo.path().file_name().unwrap().to_str().unwrap()
    ));

    let mut buf: Vec<u8> = Vec::new();
    let code = exec_in_target(&wt_path, "feat-x", &["pwd".to_string()], &mut buf)
        .expect("exec_in_target");
    assert_eq!(code, 0);
    let s = String::from_utf8(buf).expect("utf8");
    assert!(s.contains("-feat-x] "));
}

#[test]
fn exec_propagates_nonzero_exit_code() {
    let repo = TestRepo::new();
    let _wt = repo.create_worktree("feat-x");

    let wt_path = repo.path().parent().unwrap().join(format!(
        "{}-feat-x",
        repo.path().file_name().unwrap().to_str().unwrap()
    ));

    let mut buf: Vec<u8> = Vec::new();
    let code = exec_in_target(
        &wt_path,
        "feat-x",
        &["sh".to_string(), "-c".to_string(), "exit 7".to_string()],
        &mut buf,
    )
    .expect("exec_in_target");
    assert_eq!(code, 7, "exec should propagate the child's exit code verbatim");
}

#[test]
fn exec_unknown_target_errors() {
    let repo = TestRepo::new();
    // No worktree created — only the main repo exists.

    let mut buf: Vec<u8> = Vec::new();
    let result = exec_in_target(repo.path(), "nope", &["pwd".to_string()], &mut buf);

    assert!(
        result.is_err(),
        "expected Err for unknown target; got: {result:?}"
    );
    use git_worktree_manager::error::CwError;
    let err = result.unwrap_err();
    assert!(
        matches!(err, CwError::WorktreeNotFound(_)),
        "expected WorktreeNotFound, got: {err:?}"
    );
}
