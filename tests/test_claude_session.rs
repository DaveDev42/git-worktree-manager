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
    writeln!(
        f,
        r#"{{"type":"assistant","timestamp":"2026-04-25T10:00:30Z"}}"#
    )
    .unwrap();
    writeln!(
        f,
        r#"{{"type":"last-prompt","lastPrompt":"x","sessionId":"s"}}"#
    )
    .unwrap();
    writeln!(
        f,
        r#"{{"type":"permission-mode","permissionMode":"default","sessionId":"s"}}"#
    )
    .unwrap();

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
    writeln!(
        f,
        r#"{{"type":"last-prompt","lastPrompt":"x","sessionId":"s"}}"#
    )
    .unwrap();
    assert!(git_worktree_manager::operations::claude_session::newest_event_timestamp(&p).is_none());
}

use chrono::{Duration, Utc};

fn write_session_jsonl(dir: &Path, name: &str, ts: chrono::DateTime<Utc>, cwd: &str) {
    let p = dir.join(name);
    let line = format!(
        r#"{{"type":"assistant","timestamp":"{}","cwd":"{}"}}"#,
        ts.to_rfc3339(),
        cwd,
    );
    std::fs::write(p, format!("{}\n", line)).unwrap();
}

#[test]
fn find_active_sessions_returns_sessions_within_threshold() {
    use git_worktree_manager::operations::claude_session::find_active_sessions;
    let proj = tempfile::tempdir().unwrap();
    let wt_dir = tempfile::tempdir().unwrap(); // real existing dir so canonicalize succeeds
    let wt = wt_dir.path();
    let now = Utc::now();
    write_session_jsonl(
        proj.path(),
        "fresh.jsonl",
        now - Duration::minutes(2),
        wt.canonicalize().unwrap().to_str().unwrap(),
    );
    write_session_jsonl(
        proj.path(),
        "stale.jsonl",
        now - Duration::minutes(30),
        wt.canonicalize().unwrap().to_str().unwrap(),
    );

    let mut found = find_active_sessions(proj.path(), wt, Duration::minutes(10));
    found.sort_by(|a, b| a.session_id.cmp(&b.session_id));
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].session_id, "fresh");
}

#[test]
fn find_active_sessions_filters_by_cwd() {
    use git_worktree_manager::operations::claude_session::find_active_sessions;
    let proj = tempfile::tempdir().unwrap();
    let wanted_dir = tempfile::tempdir().unwrap();
    let other_dir = tempfile::tempdir().unwrap();
    let now = Utc::now();
    write_session_jsonl(
        proj.path(),
        "wrong.jsonl",
        now,
        other_dir.path().canonicalize().unwrap().to_str().unwrap(),
    );
    let found = find_active_sessions(proj.path(), wanted_dir.path(), Duration::minutes(10));
    assert!(
        found.is_empty(),
        "session for a different cwd should not match"
    );
}

#[test]
fn find_active_sessions_handles_missing_dir() {
    use git_worktree_manager::operations::claude_session::find_active_sessions;
    let found = find_active_sessions(
        Path::new("/nonexistent/dir/xyz"),
        Path::new("/tmp/x"),
        Duration::minutes(10),
    );
    assert!(found.is_empty());
}
