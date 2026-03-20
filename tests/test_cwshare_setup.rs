/// Tests for .cwshare setup functionality.
/// Covers detection, prompting state, template creation, and integration.
mod common;

use std::path::Path;

use common::TestRepo;
use git_worktree_manager::cwshare_setup;

fn init_git_repo(path: &Path) {
    use std::process::Command;
    let run = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .current_dir(path)
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@test.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@test.com")
            .output()
            .expect("Failed to run git");
    };
    run(&["init", "-b", "main"]);
    run(&["config", "user.name", "Test"]);
    run(&["config", "user.email", "test@test.com"]);
    run(&["config", "commit.gpgsign", "false"]);
    std::fs::write(path.join("README.md"), "# test\n").unwrap();
    run(&["add", "."]);
    run(&["commit", "-m", "Initial commit"]);
}

// ===========================================================================
// detect_common_files
// ===========================================================================

#[test]
fn test_detect_common_files_none() {
    let tmp = tempfile::TempDir::new().unwrap();
    let detected = cwshare_setup::detect_common_files(tmp.path());
    assert!(detected.is_empty(), "Empty dir should have no common files");
}

#[test]
fn test_detect_common_files_env() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join(".env"), "SECRET=val").unwrap();
    let detected = cwshare_setup::detect_common_files(tmp.path());
    assert!(detected.contains(&".env".to_string()));
}

#[test]
fn test_detect_common_files_env_local() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join(".env.local"), "LOCAL=val").unwrap();
    let detected = cwshare_setup::detect_common_files(tmp.path());
    assert!(detected.contains(&".env.local".to_string()));
}

#[test]
fn test_detect_common_files_multiple() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join(".env"), "A=1").unwrap();
    std::fs::write(tmp.path().join(".env.local"), "B=2").unwrap();
    std::fs::write(tmp.path().join(".env.test"), "C=3").unwrap();
    let detected = cwshare_setup::detect_common_files(tmp.path());
    assert_eq!(detected.len(), 3);
}

#[test]
fn test_detect_common_files_vscode() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join(".vscode")).unwrap();
    std::fs::write(tmp.path().join(".vscode/settings.json"), "{}").unwrap();
    let detected = cwshare_setup::detect_common_files(tmp.path());
    assert!(detected.contains(&".vscode/settings.json".to_string()));
}

#[test]
fn test_detect_common_files_config_yaml() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join("config")).unwrap();
    std::fs::write(tmp.path().join("config/local.yaml"), "key: val").unwrap();
    let detected = cwshare_setup::detect_common_files(tmp.path());
    assert!(detected.contains(&"config/local.yaml".to_string()));
}

#[test]
fn test_detect_common_files_ignores_non_common() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("README.md"), "hello").unwrap();
    std::fs::write(tmp.path().join("Cargo.toml"), "[package]").unwrap();
    let detected = cwshare_setup::detect_common_files(tmp.path());
    assert!(detected.is_empty());
}

// ===========================================================================
// has_cwshare_file
// ===========================================================================

#[test]
fn test_has_cwshare_file_false() {
    let tmp = tempfile::TempDir::new().unwrap();
    assert!(!cwshare_setup::has_cwshare_file(tmp.path()));
}

#[test]
fn test_has_cwshare_file_true() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join(".cwshare"), ".env\n").unwrap();
    assert!(cwshare_setup::has_cwshare_file(tmp.path()));
}

// ===========================================================================
// is_cwshare_prompted / mark_cwshare_prompted
// ===========================================================================

#[test]
fn test_is_cwshare_prompted_false() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_repo(tmp.path());
    assert!(!cwshare_setup::is_cwshare_prompted(tmp.path()));
}

#[test]
fn test_mark_cwshare_prompted() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_repo(tmp.path());

    assert!(!cwshare_setup::is_cwshare_prompted(tmp.path()));
    cwshare_setup::mark_cwshare_prompted(tmp.path());
    assert!(cwshare_setup::is_cwshare_prompted(tmp.path()));
}

#[test]
fn test_is_cwshare_prompted_true() {
    // After marking, should return true
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_repo(tmp.path());
    cwshare_setup::mark_cwshare_prompted(tmp.path());
    assert!(cwshare_setup::is_cwshare_prompted(tmp.path()));
}

#[test]
fn test_is_cwshare_prompted_non_git() {
    // Non-git directory should return false (git config will fail)
    let tmp = tempfile::TempDir::new().unwrap();
    assert!(!cwshare_setup::is_cwshare_prompted(tmp.path()));
}

#[test]
fn test_mark_cwshare_prompted_idempotent() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_repo(tmp.path());

    cwshare_setup::mark_cwshare_prompted(tmp.path());
    cwshare_setup::mark_cwshare_prompted(tmp.path());
    assert!(cwshare_setup::is_cwshare_prompted(tmp.path()));
}

// ===========================================================================
// create_cwshare_template
// ===========================================================================

#[test]
fn test_create_cwshare_template_no_files() {
    let tmp = tempfile::TempDir::new().unwrap();
    cwshare_setup::create_cwshare_template(tmp.path(), &[]);

    let content = std::fs::read_to_string(tmp.path().join(".cwshare")).unwrap();
    assert!(content.contains("# .cwshare"));
    assert!(content.contains("No common files detected"));
}

#[test]
fn test_create_cwshare_template_with_files() {
    let tmp = tempfile::TempDir::new().unwrap();
    let files = vec![".env".to_string(), ".env.local".to_string()];
    cwshare_setup::create_cwshare_template(tmp.path(), &files);

    let content = std::fs::read_to_string(tmp.path().join(".cwshare")).unwrap();
    assert!(content.contains("# .env"));
    assert!(content.contains("# .env.local"));
}

#[test]
fn test_cwshare_template_content_format() {
    let tmp = tempfile::TempDir::new().unwrap();
    cwshare_setup::create_cwshare_template(tmp.path(), &[]);

    let content = std::fs::read_to_string(tmp.path().join(".cwshare")).unwrap();
    // Header should explain the file
    assert!(content.contains("Files to copy to new worktrees"));
    assert!(content.contains("gw new"));
}

#[test]
fn test_cwshare_template_commented_files() {
    let tmp = tempfile::TempDir::new().unwrap();
    let files = vec!["config/local.json".to_string()];
    cwshare_setup::create_cwshare_template(tmp.path(), &files);

    let content = std::fs::read_to_string(tmp.path().join(".cwshare")).unwrap();
    // Files should be commented out (not active)
    assert!(content.contains("# config/local.json"));
    // Should contain the "Detected files" header
    assert!(content.contains("Detected files"));
}

#[test]
fn test_cwshare_template_creates_file() {
    let tmp = tempfile::TempDir::new().unwrap();
    assert!(!tmp.path().join(".cwshare").exists());
    cwshare_setup::create_cwshare_template(tmp.path(), &[]);
    assert!(tmp.path().join(".cwshare").exists());
}

#[test]
fn test_cwshare_template_overwrites() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join(".cwshare"), "old content").unwrap();
    cwshare_setup::create_cwshare_template(tmp.path(), &[]);

    let content = std::fs::read_to_string(tmp.path().join(".cwshare")).unwrap();
    assert!(!content.contains("old content"));
    assert!(content.contains("# .cwshare"));
}

// ===========================================================================
// Integration: .cwshare + worktree creation
// ===========================================================================

#[test]
fn test_cwshare_integration_copies_env() {
    // Verify that when .cwshare lists a file, it gets copied to worktrees
    // This tests the shared_files module integration
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_repo(tmp.path());

    // Create .env and .cwshare
    std::fs::write(tmp.path().join(".env"), "API_KEY=test123").unwrap();
    std::fs::write(tmp.path().join(".cwshare"), ".env\n").unwrap();

    // Use shared_files::share_files directly
    let target = tmp.path().join("worktree-dir");
    std::fs::create_dir(&target).unwrap();
    git_worktree_manager::shared_files::share_files(tmp.path(), &target);

    assert!(target.join(".env").exists());
    let content = std::fs::read_to_string(target.join(".env")).unwrap();
    assert_eq!(content, "API_KEY=test123");
}

#[test]
fn test_cwshare_integration_skips_existing() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_repo(tmp.path());

    std::fs::write(tmp.path().join(".env"), "ORIGINAL=val").unwrap();
    std::fs::write(tmp.path().join(".cwshare"), ".env\n").unwrap();

    let target = tmp.path().join("worktree-dir");
    std::fs::create_dir(&target).unwrap();
    // Pre-create the file in target
    std::fs::write(target.join(".env"), "EXISTING=val").unwrap();

    git_worktree_manager::shared_files::share_files(tmp.path(), &target);

    // Should not overwrite existing file
    let content = std::fs::read_to_string(target.join(".env")).unwrap();
    assert_eq!(content, "EXISTING=val");
}

#[test]
fn test_cwshare_integration_no_cwshare_file() {
    let tmp = tempfile::TempDir::new().unwrap();
    let target = tmp.path().join("worktree-dir");
    std::fs::create_dir(&target).unwrap();

    // No .cwshare file — should be a no-op
    git_worktree_manager::shared_files::share_files(tmp.path(), &target);

    // Target should be empty (nothing copied)
    let entries: Vec<_> = std::fs::read_dir(&target).unwrap().collect();
    assert!(entries.is_empty());
}

#[test]
fn test_cwshare_comments_ignored() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_repo(tmp.path());

    std::fs::write(tmp.path().join(".env"), "VAL=1").unwrap();
    std::fs::write(tmp.path().join(".cwshare"), "# .env\n").unwrap();

    let target = tmp.path().join("worktree-dir");
    std::fs::create_dir(&target).unwrap();

    git_worktree_manager::shared_files::share_files(tmp.path(), &target);

    // .env should NOT be copied because it's commented out
    assert!(!target.join(".env").exists());
}

#[test]
fn test_cwshare_empty_lines_ignored() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_repo(tmp.path());

    std::fs::write(tmp.path().join(".env"), "VAL=1").unwrap();
    std::fs::write(tmp.path().join(".cwshare"), "\n\n.env\n\n").unwrap();

    let target = tmp.path().join("worktree-dir");
    std::fs::create_dir(&target).unwrap();

    git_worktree_manager::shared_files::share_files(tmp.path(), &target);

    assert!(target.join(".env").exists());
}

// ===========================================================================
// Full CLI integration via TestRepo
// ===========================================================================

#[test]
fn test_cwshare_cli_worktree_copies_env() {
    // Creating a worktree via gw new with .cwshare should copy .env
    let repo = TestRepo::new();
    std::fs::write(repo.path().join(".cwshare"), ".env\n").unwrap();
    std::fs::write(repo.path().join(".env"), "SECRET=cli_test").unwrap();

    let wt = repo.create_worktree("cwshare-cli");

    assert!(
        wt.join(".env").exists(),
        ".env should be copied to worktree via .cwshare"
    );
    assert_eq!(
        std::fs::read_to_string(wt.join(".env")).unwrap(),
        "SECRET=cli_test"
    );
}

#[test]
fn test_cwshare_cli_no_cwshare_no_copy() {
    // Without .cwshare, .env should NOT be copied
    let repo = TestRepo::new();
    std::fs::write(repo.path().join(".env"), "SECRET=nope").unwrap();

    let wt = repo.create_worktree("no-cwshare-cli");

    assert!(
        !wt.join(".env").exists(),
        ".env should not appear without .cwshare"
    );
}

#[test]
fn test_cwshare_cli_multiple_files() {
    // Multiple files listed in .cwshare should all be copied
    let repo = TestRepo::new();
    std::fs::write(repo.path().join(".cwshare"), ".env\n.env.local\n").unwrap();
    std::fs::write(repo.path().join(".env"), "A=1").unwrap();
    std::fs::write(repo.path().join(".env.local"), "B=2").unwrap();

    let wt = repo.create_worktree("multi-cli");

    assert!(wt.join(".env").exists());
    assert!(wt.join(".env.local").exists());
}
