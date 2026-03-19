/// Integration tests for tree visualization — ported from Python test_tree.py (8 tests).
mod common;

use common::TestRepo;

// ===========================================================================
// 1. tree — no worktrees
// ===========================================================================

#[test]
fn test_show_tree_no_worktrees() {
    let repo = TestRepo::new();
    let output = repo.cw(&["tree"]);
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let repo_name = repo.path().file_name().unwrap().to_str().unwrap();
    assert!(
        stdout.contains(repo_name) || stdout.contains("(base repository)"),
        "Expected repo name or base indicator in output, got: {}",
        stdout
    );
    assert!(
        stdout.contains("no feature worktrees") || stdout.contains("(base repository)"),
        "Expected indication of no feature worktrees, got: {}",
        stdout
    );
}

// ===========================================================================
// 2. tree — single worktree
// ===========================================================================

#[test]
fn test_show_tree_single_worktree() {
    let repo = TestRepo::new();
    repo.create_worktree("feature-branch");

    let output = repo.cw(&["tree"]);
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let repo_name = repo.path().file_name().unwrap().to_str().unwrap();
    assert!(stdout.contains(repo_name) || stdout.contains("base repository"));
    assert!(
        stdout.contains("feature-branch"),
        "Expected 'feature-branch' in tree output"
    );
    assert!(
        stdout.contains("Legend:") || stdout.contains("clean") || stdout.contains("○"),
        "Expected legend or status indicator in output, got: {}",
        stdout
    );
}

// ===========================================================================
// 3. tree — multiple worktrees
// ===========================================================================

#[test]
fn test_show_tree_multiple_worktrees() {
    let repo = TestRepo::new();
    repo.create_worktree("feature-1");
    repo.create_worktree("feature-2");

    let output = repo.cw(&["tree"]);
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("feature-1"));
    assert!(stdout.contains("feature-2"));
    // Tree drawing characters
    assert!(
        stdout.contains("├──")
            || stdout.contains("└──")
            || stdout.contains("├")
            || stdout.contains("└"),
        "Expected tree drawing characters in output, got: {}",
        stdout
    );
}

// ===========================================================================
// 4. tree — current worktree highlighted
// ===========================================================================

#[test]
fn test_show_tree_current_worktree_highlighted() {
    let repo = TestRepo::new();
    let wt = repo.create_worktree("feature-branch");

    // Run tree from the feature worktree
    let output = TestRepo::cw_at(&wt, &["tree"]);
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.is_empty());
    // Should have a base repository header
    assert!(
        stdout.contains("(base repository)") || stdout.contains("base"),
        "Expected base repository indicator, got: {}",
        stdout
    );
}

// ===========================================================================
// 5. tree — modified worktree
// ===========================================================================

#[test]
fn test_show_tree_modified_worktree() {
    let repo = TestRepo::new();
    let wt = repo.create_worktree("feature-branch");

    // Modify a file in the worktree (without committing)
    std::fs::write(wt.join("test.txt"), "modified content").unwrap();

    let output = repo.cw(&["tree"]);
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("feature-branch"));
    // Should show modified status (either text or icon)
    assert!(
        stdout.contains("modified")
            || stdout.contains("◉")
            || stdout.contains("dirty")
            || stdout.contains("*"),
        "Expected modified indicator, got: {}",
        stdout
    );
}

// ===========================================================================
// 6. tree — sorted by branch name
// ===========================================================================

#[test]
fn test_show_tree_sorted_by_branch_name() {
    let repo = TestRepo::new();
    // Create in non-alphabetical order
    repo.create_worktree("z-feature");
    repo.create_worktree("a-feature");
    repo.create_worktree("m-feature");

    let output = repo.cw(&["tree"]);
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);

    let a_pos = stdout.find("a-feature").expect("a-feature not found");
    let m_pos = stdout.find("m-feature").expect("m-feature not found");
    let z_pos = stdout.find("z-feature").expect("z-feature not found");

    assert!(
        a_pos < m_pos && m_pos < z_pos,
        "Branches should be sorted alphabetically: a={}, m={}, z={}",
        a_pos,
        m_pos,
        z_pos
    );
}

// ===========================================================================
// 7. tree — legend present
// ===========================================================================

#[test]
fn test_show_tree_legend_present() {
    let repo = TestRepo::new();
    repo.create_worktree("feature-branch");

    let output = repo.cw(&["tree"]);
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("Legend:") || stdout.contains("legend"),
        "Expected legend section"
    );
    assert!(stdout.contains("active"), "Expected 'active' in legend");
    assert!(stdout.contains("clean"), "Expected 'clean' in legend");
    assert!(stdout.contains("modified"), "Expected 'modified' in legend");
    assert!(stdout.contains("stale"), "Expected 'stale' in legend");
    // Check for status icons
    assert!(
        stdout.contains("●") || stdout.contains("○") || stdout.contains("◉"),
        "Expected status icons, got: {}",
        stdout
    );
}

// ===========================================================================
// 8. tree — displays paths
// ===========================================================================

#[test]
fn test_show_tree_displays_paths() {
    let repo = TestRepo::new();
    let wt = repo.create_worktree("feature-branch");

    let output = repo.cw(&["tree"]);
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Path should be displayed (either relative or absolute)
    let repo_path_str = repo.path().to_str().unwrap();
    let wt_path_str = wt.to_str().unwrap();
    assert!(
        stdout.contains(repo_path_str)
            || stdout.contains(wt_path_str)
            || stdout.contains("../")
            || stdout.contains("feature-branch"),
        "Expected path information in tree output, got: {}",
        stdout
    );
}
