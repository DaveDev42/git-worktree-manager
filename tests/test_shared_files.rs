/// Tests for .cwshare file sharing functionality.
/// Ported from tests/test_shared_files.py (14 tests).
use tempfile::TempDir;

use git_worktree_manager::shared_files::{parse_cwshare, share_files};

fn tmp() -> TempDir {
    TempDir::new().unwrap()
}

#[test]
fn test_parse_cwshare_basic() {
    let tmp = tmp();
    std::fs::write(
        tmp.path().join(".cwshare"),
        ".env\n.env.local\nconfig/local.json\n",
    )
    .unwrap();
    let paths = parse_cwshare(tmp.path());
    assert_eq!(paths, vec![".env", ".env.local", "config/local.json"]);
}

#[test]
fn test_parse_cwshare_with_comments() {
    let tmp = tmp();
    std::fs::write(
        tmp.path().join(".cwshare"),
        "# This is a comment\n.env\n# Another comment\n.env.local\n",
    )
    .unwrap();
    let paths = parse_cwshare(tmp.path());
    assert_eq!(paths, vec![".env", ".env.local"]);
}

#[test]
fn test_parse_cwshare_with_empty_lines() {
    let tmp = tmp();
    std::fs::write(tmp.path().join(".cwshare"), "\n.env\n\n.env.local\n\n").unwrap();
    let paths = parse_cwshare(tmp.path());
    assert_eq!(paths, vec![".env", ".env.local"]);
}

#[test]
fn test_parse_cwshare_with_whitespace() {
    let tmp = tmp();
    std::fs::write(tmp.path().join(".cwshare"), "  .env  \n\t.env.local\t\n").unwrap();
    let paths = parse_cwshare(tmp.path());
    assert_eq!(paths, vec![".env", ".env.local"]);
}

#[test]
fn test_parse_cwshare_not_exists() {
    let tmp = tmp();
    let paths = parse_cwshare(tmp.path());
    assert!(paths.is_empty());
}

#[test]
fn test_parse_cwshare_empty_file() {
    let tmp = tmp();
    std::fs::write(tmp.path().join(".cwshare"), "").unwrap();
    let paths = parse_cwshare(tmp.path());
    assert!(paths.is_empty());
}

#[test]
fn test_share_files_copies_files() {
    let tmp = tmp();
    let source = tmp.path().join("source");
    let target = tmp.path().join("target");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::create_dir_all(&target).unwrap();

    std::fs::write(source.join(".cwshare"), ".env\n.env.local\n").unwrap();
    std::fs::write(source.join(".env"), "SECRET=value1").unwrap();
    std::fs::write(source.join(".env.local"), "SECRET=value2").unwrap();

    share_files(&source, &target);

    assert!(target.join(".env").exists());
    assert!(target.join(".env.local").exists());
    assert_eq!(
        std::fs::read_to_string(target.join(".env")).unwrap(),
        "SECRET=value1"
    );
    assert_eq!(
        std::fs::read_to_string(target.join(".env.local")).unwrap(),
        "SECRET=value2"
    );
    assert!(!target.join(".env").is_symlink());
    assert!(!target.join(".env.local").is_symlink());
}

#[test]
fn test_share_files_copies_directories() {
    let tmp = tmp();
    let source = tmp.path().join("source");
    let target = tmp.path().join("target");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::create_dir_all(&target).unwrap();

    std::fs::write(source.join(".cwshare"), "config\n").unwrap();
    let config = source.join("config");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(config.join("local.json"), r#"{"key": "value"}"#).unwrap();
    std::fs::write(config.join("secrets.json"), r#"{"secret": "hidden"}"#).unwrap();

    share_files(&source, &target);

    let tc = target.join("config");
    assert!(tc.exists());
    assert!(tc.is_dir());
    assert!(!tc.is_symlink());
    assert_eq!(
        std::fs::read_to_string(tc.join("local.json")).unwrap(),
        r#"{"key": "value"}"#
    );
    assert_eq!(
        std::fs::read_to_string(tc.join("secrets.json")).unwrap(),
        r#"{"secret": "hidden"}"#
    );
}

#[test]
fn test_share_files_nested_path() {
    let tmp = tmp();
    let source = tmp.path().join("source");
    let target = tmp.path().join("target");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::create_dir_all(&target).unwrap();

    std::fs::write(source.join(".cwshare"), "config/local/settings.json\n").unwrap();
    let nested = source.join("config").join("local");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(nested.join("settings.json"), r#"{"nested": true}"#).unwrap();

    share_files(&source, &target);

    let target_file = target.join("config").join("local").join("settings.json");
    assert!(target_file.exists());
    assert_eq!(
        std::fs::read_to_string(&target_file).unwrap(),
        r#"{"nested": true}"#
    );
}

#[test]
fn test_share_files_skips_nonexistent_source() {
    let tmp = tmp();
    let source = tmp.path().join("source");
    let target = tmp.path().join("target");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::create_dir_all(&target).unwrap();

    std::fs::write(source.join(".cwshare"), ".env\n.env.local\n").unwrap();
    std::fs::write(source.join(".env"), "SECRET=value").unwrap();
    // .env.local not created

    share_files(&source, &target);

    assert!(target.join(".env").exists());
    assert!(!target.join(".env.local").exists());
}

#[test]
fn test_share_files_skips_existing_target() {
    let tmp = tmp();
    let source = tmp.path().join("source");
    let target = tmp.path().join("target");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::create_dir_all(&target).unwrap();

    std::fs::write(source.join(".cwshare"), ".env\n").unwrap();
    std::fs::write(source.join(".env"), "NEW_SECRET=new_value").unwrap();
    std::fs::write(target.join(".env"), "OLD_SECRET=old_value").unwrap();

    share_files(&source, &target);

    assert_eq!(
        std::fs::read_to_string(target.join(".env")).unwrap(),
        "OLD_SECRET=old_value"
    );
}

#[test]
fn test_share_files_no_cwshare() {
    let tmp = tmp();
    let source = tmp.path().join("source");
    let target = tmp.path().join("target");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::create_dir_all(&target).unwrap();

    std::fs::write(source.join(".env"), "SECRET=value").unwrap();

    share_files(&source, &target);

    assert!(!target.join(".env").exists());
}

#[test]
fn test_share_files_empty_cwshare() {
    let tmp = tmp();
    let source = tmp.path().join("source");
    let target = tmp.path().join("target");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::create_dir_all(&target).unwrap();

    std::fs::write(source.join(".cwshare"), "").unwrap();
    std::fs::write(source.join(".env"), "SECRET=value").unwrap();

    share_files(&source, &target);

    assert!(!target.join(".env").exists());
}

#[cfg(unix)]
#[test]
fn test_share_files_preserves_symlinks_in_copied_dirs() {
    let tmp = tmp();
    let source = tmp.path().join("source");
    let target = tmp.path().join("target");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::create_dir_all(&target).unwrap();

    std::fs::write(source.join(".cwshare"), "config\n").unwrap();
    let config = source.join("config");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(config.join("real_file.json"), r#"{"real": true}"#).unwrap();
    std::os::unix::fs::symlink(config.join("real_file.json"), config.join("link_file.json"))
        .unwrap();

    share_files(&source, &target);

    assert!(target.join("config").join("link_file.json").is_symlink());
}
