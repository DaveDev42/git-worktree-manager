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

#[test]
fn run_only_filters_by_glob() {
    let repo = TestRepo::new();
    let _wt_x = repo.create_worktree("feat-x");
    let _wt_y = repo.create_worktree("bug-y");

    let wt_x_path = repo.path().parent().unwrap().join(format!(
        "{}-feat-x",
        repo.path().file_name().unwrap().to_str().unwrap()
    ));

    let mut buf: Vec<u8> = Vec::new();
    let code = run_in_scope_to_writer(
        &wt_x_path,
        &["pwd".to_string()],
        Some("*-feat-*"),
        false,
        1,
        false,
        &mut buf,
    )
    .expect("run_in_scope_to_writer");

    assert_eq!(code, 0);
    let s = String::from_utf8(buf).expect("utf8");

    // feat-x is included
    assert!(
        s.lines().any(|l| l.contains("-feat-x] ")),
        "feat-x should match '*-feat-*' glob; got: {s}"
    );
    // bug-y is excluded
    assert!(
        s.lines().all(|l| !l.contains("-bug-y] ")),
        "bug-y should be filtered out by '*-feat-*' glob; got: {s}"
    );
}

#[test]
fn run_no_main_skips_main_worktree() {
    let repo = TestRepo::new();
    let _wt_x = repo.create_worktree("feat-x");

    let wt_x_path = repo.path().parent().unwrap().join(format!(
        "{}-feat-x",
        repo.path().file_name().unwrap().to_str().unwrap()
    ));

    let main_basename = repo.path().file_name().unwrap().to_str().unwrap().to_string();

    let mut buf: Vec<u8> = Vec::new();
    let code = run_in_scope_to_writer(
        &wt_x_path,
        &["pwd".to_string()],
        None,
        true, // no_main
        1,
        false,
        &mut buf,
    )
    .expect("run_in_scope_to_writer");

    assert_eq!(code, 0);
    let s = String::from_utf8(buf).expect("utf8");

    // The main worktree's name is the TempDir basename (no -feat-x suffix).
    // Its prefix would be `[<tempdir-basename>] `. Assert it does NOT appear.
    let main_prefix = format!("[{}] ", main_basename);
    assert!(
        s.lines().all(|l| !l.starts_with(&main_prefix)),
        "main worktree (`{main_prefix}`) should be skipped; got: {s}"
    );
    // feat-x still ran
    assert!(
        s.lines().any(|l| l.contains("-feat-x] ")),
        "feat-x should still run; got: {s}"
    );
}

#[test]
fn run_parallel_keeps_output_per_worktree_contiguous() {
    let repo = TestRepo::new();
    let _wt_a = repo.create_worktree("a");
    let _wt_b = repo.create_worktree("b");
    let _wt_c = repo.create_worktree("c");

    let main_basename = repo.path().file_name().unwrap().to_str().unwrap().to_string();
    let wt_a_path = repo.path().parent().unwrap().join(format!("{}-a", main_basename));

    let mut buf: Vec<u8> = Vec::new();
    let code = run_in_scope_to_writer(
        &wt_a_path,
        &[
            "sh".to_string(),
            "-c".to_string(),
            "echo line1; echo line2".to_string(),
        ],
        None,
        false,
        4, // jobs
        false,
        &mut buf,
    )
    .expect("run_in_scope_to_writer");

    assert_eq!(code, 0);
    let s = String::from_utf8(buf).expect("utf8");

    // For every worktree, the two lines must appear back-to-back (contiguous).
    // Feature worktrees have -a / -b / -c suffix.
    for suffix in &["-a", "-b", "-c"] {
        let lines: Vec<&str> = s.lines().collect();
        let mut found = false;
        for (idx, line) in lines.iter().enumerate() {
            if line.ends_with(&format!("{}] line1", suffix)) {
                let prefix_part = &line[..line.len() - "line1".len()];
                let next_expected = format!("{}line2", prefix_part);
                assert!(
                    idx + 1 < lines.len() && lines[idx + 1] == next_expected,
                    "expected line2 immediately after line1 for {suffix}; got: {s}"
                );
                found = true;
                break;
            }
        }
        assert!(found, "did not find line1 for worktree suffix {suffix}; got: {s}");
    }

    // Main worktree (no suffix; basename only).
    let main_prefix = format!("[{}] ", main_basename);
    let main_line1 = format!("{}line1", main_prefix);
    let main_line2 = format!("{}line2", main_prefix);
    let lines: Vec<&str> = s.lines().collect();
    let mut found_main = false;
    for (idx, line) in lines.iter().enumerate() {
        if *line == main_line1 {
            assert!(
                idx + 1 < lines.len() && lines[idx + 1] == main_line2,
                "main line2 should follow line1; got: {s}"
            );
            found_main = true;
            break;
        }
    }
    assert!(found_main, "main worktree line1 missing; got: {s}");
}
