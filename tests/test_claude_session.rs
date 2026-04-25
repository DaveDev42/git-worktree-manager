use git_worktree_manager::operations::claude_session::encode_project_dir;
use std::io::Write;
use std::path::Path;

#[test]
fn encode_simple_path() {
    let p = Path::new("/Users/dave/Projects/github.com/git-worktree-manager");
    assert_eq!(
        encode_project_dir(p),
        "-Users-dave-Projects-github-com-git-worktree-manager"
    );
}

#[test]
fn encode_path_with_dots() {
    let p = Path::new("/Users/dave/Projects/github.com/foo.bar");
    assert_eq!(
        encode_project_dir(p),
        "-Users-dave-Projects-github-com-foo-bar"
    );
}

#[test]
fn encode_path_with_trailing_slash() {
    let p = Path::new("/tmp/foo/");
    assert_eq!(encode_project_dir(p), "-tmp-foo");
}

#[test]
fn newest_event_timestamp_skips_metadata_trailers() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("session.jsonl");
    let mut f = std::fs::File::create(&p).unwrap();
    writeln!(f, r#"{{"type":"user","timestamp":"2026-04-25T10:00:00Z"}}"#).unwrap();
    writeln!(f, r#"{{"type":"assistant","timestamp":"2026-04-25T10:00:30Z"}}"#).unwrap();
    writeln!(f, r#"{{"type":"last-prompt","lastPrompt":"x","sessionId":"s"}}"#).unwrap();
    writeln!(f, r#"{{"type":"permission-mode","permissionMode":"default","sessionId":"s"}}"#).unwrap();

    let ts = git_worktree_manager::operations::claude_session::newest_event_timestamp(&p)
        .expect("should parse");
    assert_eq!(ts.timestamp(), 1777111230); // 2026-04-25T10:00:30Z
}

#[test]
fn newest_event_timestamp_returns_none_for_empty() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("empty.jsonl");
    std::fs::write(&p, b"").unwrap();
    assert!(git_worktree_manager::operations::claude_session::newest_event_timestamp(&p).is_none());
}

#[test]
fn newest_event_timestamp_returns_none_for_metadata_only() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("meta.jsonl");
    let mut f = std::fs::File::create(&p).unwrap();
    writeln!(f, r#"{{"type":"last-prompt","lastPrompt":"x","sessionId":"s"}}"#).unwrap();
    assert!(git_worktree_manager::operations::claude_session::newest_event_timestamp(&p).is_none());
}
