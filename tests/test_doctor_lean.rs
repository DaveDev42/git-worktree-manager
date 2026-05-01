//! Phase 8.3: gw doctor is now 5 lean checks. Verify the deleted checks
//! ("behind base", "merge conflicts") no longer appear, and that the new
//! "busy worktrees" check is present.

mod common;
use common::TestRepo;

#[test]
fn doctor_does_not_run_behind_base_check() {
    let repo = TestRepo::new();
    let output = repo.cw_stdout(&["doctor"]);
    assert!(
        !output.contains("behind base"),
        "doctor should no longer mention behind-base check; got:\n{}",
        output
    );
}

#[test]
fn doctor_does_not_run_merge_conflicts_check() {
    let repo = TestRepo::new();
    let output = repo.cw_stdout(&["doctor"]);
    assert!(
        !output.contains("merge conflicts") && !output.contains("Checking for merge conflicts"),
        "doctor should no longer mention merge-conflicts check; got:\n{}",
        output
    );
}

#[test]
fn doctor_runs_busy_check() {
    let repo = TestRepo::new();
    let output = repo.cw_stdout(&["doctor"]);
    assert!(
        output.contains("busy"),
        "doctor should mention busy worktrees; got:\n{}",
        output
    );
}

#[test]
fn doctor_has_five_numbered_sections() {
    let repo = TestRepo::new();
    let output = repo.cw_stdout(&["doctor"]);
    // Match "1.", "2.", ... "5." but NOT "6.".
    for n in 1..=5 {
        let prefix = format!("{}. ", n);
        assert!(
            output.contains(&prefix),
            "doctor missing section '{}' in output:\n{}",
            prefix,
            output
        );
    }
    assert!(
        !output.contains("6. "),
        "doctor should have only 5 sections; found 6:\n{}",
        output
    );
}
