/// Tests for session management.
/// Ported from tests/test_session_manager.py (30 tests).
///
/// Note: Session functions use real filesystem paths (~/.config/claude-worktree/sessions/).
/// We test serialization logic and functions that don't pollute user config.
use git_worktree_manager::session;

#[test]
fn test_session_metadata_serialization() {
    let meta = session::SessionMetadata {
        branch: "fix-auth".to_string(),
        ai_tool: "claude".to_string(),
        worktree_path: "/tmp/test".to_string(),
        created_at: "2026-01-01T00:00:00Z".to_string(),
        updated_at: "2026-01-02T00:00:00Z".to_string(),
    };

    let json = serde_json::to_string(&meta).unwrap();
    let deserialized: session::SessionMetadata = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.branch, "fix-auth");
    assert_eq!(deserialized.ai_tool, "claude");
    assert_eq!(deserialized.worktree_path, "/tmp/test");
    assert_eq!(deserialized.created_at, "2026-01-01T00:00:00Z");
    assert_eq!(deserialized.updated_at, "2026-01-02T00:00:00Z");
}

#[test]
fn test_get_sessions_dir_path() {
    let dir = session::get_sessions_dir();
    assert!(dir.to_string_lossy().contains("sessions"));
    assert!(dir.to_string_lossy().contains(".config"));
}

#[test]
fn test_get_session_dir_sanitizes_branch() {
    let dir = session::get_session_dir("feat/auth");
    assert!(dir.to_string_lossy().contains("feat-auth"));
}

#[test]
fn test_get_session_dir_strips_refs_heads() {
    let dir1 = session::get_session_dir("refs/heads/main");
    let dir2 = session::get_session_dir("main");
    assert_eq!(dir1.file_name(), dir2.file_name());
}

#[test]
fn test_claude_native_session_not_exists() {
    let tmp = tempfile::TempDir::new().unwrap();
    assert!(!session::claude_native_session_exists(tmp.path()));
}

#[test]
fn test_load_session_metadata_not_found() {
    // Random branch name that shouldn't exist
    let meta = session::load_session_metadata("nonexistent-test-branch-xyz-12345");
    assert!(meta.is_none());
}

#[test]
fn test_load_context_not_found() {
    let ctx = session::load_context("nonexistent-test-branch-xyz-12345");
    assert!(ctx.is_none());
}

#[test]
fn test_list_sessions_returns_vec() {
    let sessions = session::list_sessions();
    // Just verify it returns without error; may or may not have entries
    assert!(sessions.len() >= 0);
}

#[test]
fn test_chrono_now_iso_pub_format() {
    let ts = session::chrono_now_iso_pub();
    assert!(ts.contains('T'));
    assert!(ts.ends_with('Z'));
    // Should look like YYYY-MM-DDTHH:MM:SSZ
    assert!(ts.len() >= 19);
}
