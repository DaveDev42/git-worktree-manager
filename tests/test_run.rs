//! Integration tests for `gw run` — fan-out command execution.

mod common;
use common::TestRepo;

use git_worktree_manager::operations::run::run_in_scope_to_writer;

#[test]
fn run_executes_cmd_in_each_worktree_with_prefix() {
    let repo = TestRepo::new();
    let _wt_x = repo.create_worktree("feat-x");

    // cwd inside the feat-x worktree
    let wt_x_path = repo.path().parent().unwrap().join(format!(
        "{}-feat-x",
        repo.path().file_name().unwrap().to_str().unwrap()
    ));

    let mut buf: Vec<u8> = Vec::new();
    let code = run_in_scope_to_writer(
        &wt_x_path,
        &["pwd".to_string()],
        None,
        false,
        1,
        false,
        &mut buf,
    )
    .expect("run_in_scope_to_writer");

    assert_eq!(code, 0, "all-zero exits expected from pwd");
    let s = String::from_utf8(buf).expect("utf8");

    // Both worktrees ran. The main repo's basename is unpredictable (TempDir)
    // so we assert presence of the [feat-x] prefix and the count of [<name>] occurrences.
    assert!(s.contains("[") && s.contains("] "), "should be prefixed; got: {s}");
    assert!(s.ends_with('\n'), "trailing newline expected");

    // feat-x basename ends with "-feat-x"
    let feat_x_lines = s.lines().filter(|l| l.contains("-feat-x] ")).count();
    assert!(feat_x_lines >= 1, "feat-x prefix should appear at least once; got: {s}");

    // Main worktree appears as the other prefixed line(s).
    let prefix_lines = s.lines().filter(|l| l.starts_with('[')).count();
    assert!(prefix_lines >= 2, "main + feat-x = at least 2 prefixed lines; got: {s}");
}
