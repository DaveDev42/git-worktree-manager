//! Verifies write_if_changed honors content equality and creates parents,
//! and that sentinel write/check round-trips.

use git_worktree_manager::operations::setup_claude::writer;
use std::fs;

#[test]
fn write_if_changed_creates_file_and_parents() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("a/b/c/file.txt");
    let changed = writer::write_if_changed(&target, "hello").unwrap();
    assert!(changed, "first write should report changed");
    assert_eq!(fs::read_to_string(&target).unwrap(), "hello");
}

#[test]
fn write_if_changed_skips_when_content_matches() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("file.txt");
    writer::write_if_changed(&target, "hello").unwrap();
    let mtime_1 = fs::metadata(&target).unwrap().modified().unwrap();

    std::thread::sleep(std::time::Duration::from_millis(20));
    let changed = writer::write_if_changed(&target, "hello").unwrap();
    let mtime_2 = fs::metadata(&target).unwrap().modified().unwrap();

    assert!(!changed, "no-change write should report unchanged");
    assert_eq!(mtime_1, mtime_2, "no-change write must not touch mtime");
}

#[test]
fn write_if_changed_rewrites_on_diff() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("file.txt");
    writer::write_if_changed(&target, "v1").unwrap();
    let changed = writer::write_if_changed(&target, "v2").unwrap();
    assert!(changed);
    assert_eq!(fs::read_to_string(&target).unwrap(), "v2");
}

#[test]
fn sentinel_round_trip() {
    let tmp = tempfile::tempdir().unwrap();
    let sentinel = tmp.path().join(".gw-managed");
    assert!(!writer::sentinel_present(&sentinel));
    writer::write_sentinel(&sentinel).unwrap();
    assert!(writer::sentinel_present(&sentinel));
}
