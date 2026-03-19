/// Integration tests for stats functionality — ported from Python test_stats.py (13 tests).
mod common;

use common::TestRepo;
use git_worktree_manager::operations::display::format_age;

// ===========================================================================
// 1. stats — no worktrees
// ===========================================================================

#[test]
fn test_show_stats_no_worktrees() {
    let repo = TestRepo::new();
    let stdout = repo.cw_stdout(&["stats"]);
    assert!(stdout.contains("No feature worktrees found"));
}

// ===========================================================================
// 2. stats — single worktree
// ===========================================================================

#[test]
fn test_show_stats_single_worktree() {
    let repo = TestRepo::new();
    repo.create_worktree("feature-branch");

    let stdout = repo.cw_stdout(&["stats"]);
    // Check for statistics sections
    assert!(
        stdout.contains("Worktree Statistics")
            || stdout.contains("Statistics")
            || stdout.contains("📊"),
        "Expected statistics header, got: {}",
        stdout
    );
    assert!(
        stdout.contains("Overview:") || stdout.contains("Overview"),
        "Expected 'Overview' section, got: {}",
        stdout
    );
    assert!(
        stdout.contains("Total worktrees: 1") || stdout.contains("Total worktrees:  1"),
        "Expected 'Total worktrees: 1', got: {}",
        stdout
    );
    assert!(
        stdout.contains("feature-branch"),
        "Expected branch name in stats output"
    );
}

// ===========================================================================
// 3. stats — multiple worktrees
// ===========================================================================

#[test]
fn test_show_stats_multiple_worktrees() {
    let repo = TestRepo::new();
    repo.create_worktree("feature-1");
    repo.create_worktree("feature-2");
    repo.create_worktree("feature-3");

    let stdout = repo.cw_stdout(&["stats"]);
    assert!(
        stdout.contains("Total worktrees: 3") || stdout.contains("Total worktrees:  3"),
        "Expected 'Total worktrees: 3', got: {}",
        stdout
    );
}

// ===========================================================================
// 4. stats — age statistics
// ===========================================================================

#[test]
fn test_show_stats_age_statistics() {
    let repo = TestRepo::new();
    repo.create_worktree("feature-branch");

    let stdout = repo.cw_stdout(&["stats"]);
    assert!(
        stdout.contains("Age Statistics:") || stdout.contains("Age"),
        "Expected age statistics section, got: {}",
        stdout
    );
    assert!(
        stdout.contains("Average age:") || stdout.contains("average"),
        "Expected 'Average age' in stats, got: {}",
        stdout
    );
    assert!(
        stdout.contains("Oldest:") || stdout.contains("oldest"),
        "Expected 'Oldest' in stats, got: {}",
        stdout
    );
    assert!(
        stdout.contains("Newest:") || stdout.contains("newest"),
        "Expected 'Newest' in stats, got: {}",
        stdout
    );
}

// ===========================================================================
// 5. stats — commit statistics
// ===========================================================================

#[test]
fn test_show_stats_commit_statistics() {
    let repo = TestRepo::new();
    let wt = repo.create_worktree("feature-branch");

    // Make a commit in the feature worktree
    TestRepo::commit_file_at(&wt, "test.txt", "test", "test commit");

    let stdout = repo.cw_stdout(&["stats"]);
    assert!(
        stdout.contains("Commit Statistics:")
            || stdout.contains("Commit")
            || stdout.contains("commit"),
        "Expected commit statistics section, got: {}",
        stdout
    );
    assert!(
        stdout.contains("Total commits")
            || stdout.contains("total commits")
            || stdout.contains("commits"),
        "Expected total commits info, got: {}",
        stdout
    );
    assert!(
        stdout.contains("Average commits")
            || stdout.contains("average")
            || stdout.contains("Average"),
        "Expected average commits info, got: {}",
        stdout
    );
}

// ===========================================================================
// 6. stats — status distribution
// ===========================================================================

#[test]
fn test_show_stats_status_distribution() {
    let repo = TestRepo::new();

    // Create clean worktree
    repo.create_worktree("clean-branch");

    // Create modified worktree
    let modified_wt = repo.create_worktree("modified-branch");
    std::fs::write(modified_wt.join("test.txt"), "modified").unwrap();

    let stdout = repo.cw_stdout(&["stats"]);
    assert!(
        stdout.contains("Status:") || stdout.contains("status"),
        "Expected status distribution section, got: {}",
        stdout
    );
    assert!(
        stdout.contains("clean"),
        "Expected 'clean' status, got: {}",
        stdout
    );
}

// ===========================================================================
// 7. stats — oldest worktrees
// ===========================================================================

#[test]
fn test_show_stats_oldest_worktrees() {
    let repo = TestRepo::new();

    for i in 0..3 {
        repo.create_worktree(&format!("feature-{}", i));
    }

    let stdout = repo.cw_stdout(&["stats"]);
    assert!(
        stdout.contains("Oldest Worktrees:")
            || stdout.contains("Oldest")
            || stdout.contains("oldest"),
        "Expected oldest worktrees section, got: {}",
        stdout
    );
}

// ===========================================================================
// 8. stats — most active worktrees
// ===========================================================================

#[test]
fn test_show_stats_most_active_worktrees() {
    let repo = TestRepo::new();

    for i in 0..2 {
        let wt = repo.create_worktree(&format!("feature-{}", i));

        // Make commits (more for higher index)
        for j in 0..=i {
            TestRepo::commit_file_at(
                &wt,
                &format!("test{}.txt", j),
                &format!("test {}", j),
                &format!("commit {}", j),
            );
        }
    }

    let stdout = repo.cw_stdout(&["stats"]);
    assert!(
        stdout.contains("Most Active Worktrees")
            || stdout.contains("Most Active")
            || stdout.contains("active"),
        "Expected most active worktrees section, got: {}",
        stdout
    );
}

// ===========================================================================
// 9. format_age — hours
// ===========================================================================

#[test]
fn test_format_age_hours() {
    assert_eq!(format_age(0.5), "12h ago");
    assert_eq!(format_age(0.04), "just now"); // Less than 1 hour
    assert_eq!(format_age(0.0), "just now");
}

// ===========================================================================
// 10. format_age — days
// ===========================================================================

#[test]
fn test_format_age_days() {
    assert_eq!(format_age(1.5), "1d ago");
    assert_eq!(format_age(3.0), "3d ago");
    assert_eq!(format_age(6.9), "6d ago");
}

// ===========================================================================
// 11. format_age — weeks
// ===========================================================================

#[test]
fn test_format_age_weeks() {
    assert_eq!(format_age(7.0), "1w ago");
    assert_eq!(format_age(14.0), "2w ago");
    assert_eq!(format_age(21.0), "3w ago");
}

// ===========================================================================
// 12. format_age — months
// ===========================================================================

#[test]
fn test_format_age_months() {
    assert_eq!(format_age(30.0), "1mo ago");
    assert_eq!(format_age(60.0), "2mo ago");
    assert_eq!(format_age(180.0), "6mo ago");
}

// ===========================================================================
// 13. format_age — years
// ===========================================================================

#[test]
fn test_format_age_years() {
    assert_eq!(format_age(365.0), "1y ago");
    assert_eq!(format_age(730.0), "2y ago");
}
