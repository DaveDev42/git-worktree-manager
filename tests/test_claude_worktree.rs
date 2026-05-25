//! Integration tests for `gw _claude-worktree-create` and
//! `gw _claude-worktree-remove`.
//!
//! These subcommands are the internal handlers for Claude Code's
//! `WorktreeCreate` / `WorktreeRemove` hook events.
//!
//! Unix-only for the pre_rm sentinel tests (they rely on `touch`).

mod common;
use common::TestRepo;
use std::io::Write;
use std::process::{Command, Stdio};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Converts a path to a forward-slash string safe for embedding in JSON.
///
/// On Windows, `Path::display()` produces backslashes which are invalid JSON
/// escape sequences. Replacing them with forward slashes produces valid JSON
/// that the Rust JSON parser (and Windows APIs) accept.
fn json_path(p: &std::path::Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// Run `gw _claude-worktree-create` with the given stdin payload, inside
/// `cwd`. Returns the full `Output`.
fn run_create_with_stdin(cwd: &std::path::Path, stdin_payload: &str) -> std::process::Output {
    let mut child = Command::new(TestRepo::cw_bin())
        .arg("_claude-worktree-create")
        .current_dir(cwd)
        .env("CW_LAUNCH_METHOD", "foreground")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn _claude-worktree-create");

    child
        .stdin
        .as_mut()
        .expect("stdin handle")
        .write_all(stdin_payload.as_bytes())
        .expect("write stdin");

    child.wait_with_output().expect("wait for child")
}

/// Run `gw _claude-worktree-remove` with the given stdin payload, inside
/// `cwd`. Returns the full `Output`.
fn run_remove_with_stdin(cwd: &std::path::Path, stdin_payload: &str) -> std::process::Output {
    let mut child = Command::new(TestRepo::cw_bin())
        .arg("_claude-worktree-remove")
        .current_dir(cwd)
        .env("CW_LAUNCH_METHOD", "foreground")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn _claude-worktree-remove");

    child
        .stdin
        .as_mut()
        .expect("stdin handle")
        .write_all(stdin_payload.as_bytes())
        .expect("write stdin");

    child.wait_with_output().expect("wait for child")
}

// ===========================================================================
// _claude-worktree-create — happy path
// ===========================================================================

/// Assert the inner branch was created in `repo` and return its worktree path
/// line from the hook's stdout.
fn assert_branch_created(out: &std::process::Output, repo: &TestRepo, branch: &str) -> String {
    assert!(
        out.status.success(),
        "_claude-worktree-create should succeed. stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(
        lines.len(),
        1,
        "stdout must be exactly one line, got: {:?}",
        stdout
    );

    let wt_path = std::path::Path::new(lines[0]);
    assert!(
        wt_path.is_absolute(),
        "output path must be absolute, got: {}",
        lines[0]
    );
    assert!(
        wt_path.is_dir(),
        "worktree directory must exist at {}, but it does not",
        lines[0]
    );

    let branches = Command::new("git")
        .args([
            "-C",
            &repo.path().to_string_lossy(),
            "branch",
            "--list",
            branch,
        ])
        .output()
        .expect("git branch --list");
    assert!(
        String::from_utf8_lossy(&branches.stdout).contains(branch),
        "branch '{}' should have been created from worktree_name",
        branch
    );

    lines[0].to_string()
}

/// Claude Code's real WorktreeCreate schema (`worktree_name` + `cwd` +
/// `base_commit`, NO `worktree_path`) → the hook creates the worktree itself,
/// names the branch after `worktree_name`, and prints the resulting path.
#[test]
fn create_uses_worktree_name_as_branch() {
    let repo = TestRepo::new();
    let branch = "claude-create-test";
    let payload = format!(
        r#"{{"hook_event_name":"WorktreeCreate","session_id":"s1","cwd":"{cwd}","worktree_name":"{branch}","base_commit":"main","isolation_type":"worktree"}}"#,
        cwd = json_path(repo.path()),
    );

    let out = run_create_with_stdin(repo.path(), &payload);
    assert_branch_created(&out, &repo, branch);
}

/// The `name` and `base_ref` keys are accepted as aliases for `worktree_name`
/// and `base_commit` (some Claude Code builds / older drafts emit them; the
/// empirically captured payload used `name`).
#[test]
fn create_accepts_name_and_base_ref_aliases() {
    let repo = TestRepo::new();
    let branch = "claude-name-alias";
    let payload = format!(
        r#"{{"hook_event_name":"WorktreeCreate","session_id":"s1","transcript_path":"/tmp/t.jsonl","cwd":"{cwd}","name":"{branch}","base_ref":"main"}}"#,
        cwd = json_path(repo.path()),
    );

    let out = run_create_with_stdin(repo.path(), &payload);
    assert_branch_created(&out, &repo, branch);
}

/// An explicit `worktree_path` (legacy/optional) is honored: the worktree
/// lands at exactly that path rather than gw's default convention.
#[test]
fn create_honors_explicit_worktree_path() {
    let repo = TestRepo::new();
    let branch = "claude-explicit-path";
    let parent = tempfile::tempdir().expect("worktree parent tempdir");
    let wt = parent.path().join(branch);
    let payload = format!(
        r#"{{"hook_event_name":"WorktreeCreate","cwd":"{cwd}","worktree_name":"{branch}","worktree_path":"{wt}"}}"#,
        cwd = json_path(repo.path()),
        wt = json_path(&wt),
    );

    let out = run_create_with_stdin(repo.path(), &payload);
    let line = assert_branch_created(&out, &repo, branch);

    let got = std::path::Path::new(&line);
    assert_eq!(
        got.canonicalize().unwrap_or_else(|_| got.to_path_buf()),
        wt.canonicalize().unwrap_or(wt),
        "worktree must land at the explicit path when one is supplied"
    );
}

/// Unknown extra fields in the payload must be ignored (forward-compatibility).
#[test]
fn create_ignores_unknown_fields() {
    let repo = TestRepo::new();
    let branch = "claude-unknown-fields";
    let payload = format!(
        r#"{{"hook_event_name":"WorktreeCreate","unknown_future_field":42,"cwd":"{cwd}","worktree_name":"{branch}"}}"#,
        cwd = json_path(repo.path()),
    );

    let out = run_create_with_stdin(repo.path(), &payload);
    assert_branch_created(&out, &repo, branch);
}

/// `base_commit` is optional — omitting it falls back to the default base.
#[test]
fn create_without_base_commit_uses_default_base() {
    let repo = TestRepo::new();
    let branch = "claude-no-base-commit";
    let payload = format!(
        r#"{{"hook_event_name":"WorktreeCreate","cwd":"{cwd}","worktree_name":"{branch}"}}"#,
        cwd = json_path(repo.path()),
    );

    let out = run_create_with_stdin(repo.path(), &payload);
    assert_branch_created(&out, &repo, branch);
}

// ===========================================================================
// _claude-worktree-create — error cases
// ===========================================================================

/// Malformed JSON → non-zero exit, error on stderr.
#[test]
fn create_invalid_json_exits_nonzero() {
    let repo = TestRepo::new();
    let out = run_create_with_stdin(repo.path(), "not-json-at-all");

    assert!(
        !out.status.success(),
        "invalid JSON should cause non-zero exit"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.is_empty(),
        "stderr should contain an error message, got empty"
    );
}

/// Missing `worktree_name` (and its `name` alias) → non-zero exit; it is the
/// only required field in Claude Code's WorktreeCreate payload.
#[test]
fn create_missing_worktree_name_exits_nonzero() {
    let repo = TestRepo::new();
    // Neither `worktree_name` nor `name` present; serde should fail.
    let payload = format!(
        r#"{{"hook_event_name":"WorktreeCreate","cwd":"{}"}}"#,
        json_path(repo.path())
    );

    let out = run_create_with_stdin(repo.path(), &payload);
    assert!(
        !out.status.success(),
        "missing worktree_name should cause non-zero exit"
    );
}

/// An empty `worktree_name` → non-zero exit (cannot derive a branch).
#[test]
fn create_empty_worktree_name_exits_nonzero() {
    let repo = TestRepo::new();
    let payload = format!(
        r#"{{"hook_event_name":"WorktreeCreate","cwd":"{}","worktree_name":""}}"#,
        json_path(repo.path())
    );

    let out = run_create_with_stdin(repo.path(), &payload);
    assert!(
        !out.status.success(),
        "empty worktree_name should cause non-zero exit"
    );
}

/// Empty stdin → non-zero exit.
#[test]
fn create_empty_stdin_exits_nonzero() {
    let repo = TestRepo::new();
    let out = run_create_with_stdin(repo.path(), "");

    assert!(
        !out.status.success(),
        "empty stdin should cause non-zero exit"
    );
}

// ===========================================================================
// _claude-worktree-remove — happy path (no hook configured)
// ===========================================================================

/// Payload with a valid `worktree_path`, no hook configured → exit 0, no stdout.
#[test]
fn remove_no_hook_exits_zero_silently() {
    let repo = TestRepo::new();
    let wt_path = repo.create_worktree("claude-remove-nohook");

    let payload = format!(
        r#"{{"hook_event_name":"WorktreeRemove","session_id":"s1","cwd":"{repo}","worktree_path":"{wt}","isolation_type":"worktree"}}"#,
        repo = json_path(repo.path()),
        wt = json_path(&wt_path)
    );

    let out = run_remove_with_stdin(repo.path(), &payload);
    assert!(
        out.status.success(),
        "_claude-worktree-remove should exit 0; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.trim().is_empty(),
        "stdout must be empty, got: {:?}",
        stdout
    );
}

// ===========================================================================
// _claude-worktree-remove — pre_rm hook execution
// ===========================================================================

/// When a pre_rm hook is configured, it fires for the given worktree.
#[cfg(unix)]
#[test]
fn remove_pre_rm_hook_fires_for_worktree() {
    let repo = TestRepo::new();
    let wt_path = repo.create_worktree("claude-remove-hook-fires");

    let marker_dir = tempfile::tempdir().expect("marker tempdir");
    let marker = marker_dir.path().join("pre_rm_fired");

    // Write .cwconfig.json to the main repo so hooks::run_event picks it up.
    let cfg = format!(
        r#"{{"hooks":{{"pre_rm":"touch '{}'"}} }}"#,
        marker.display()
    );
    std::fs::write(repo.path().join(".cwconfig.json"), &cfg).unwrap();

    let payload = format!(
        r#"{{"hook_event_name":"WorktreeRemove","cwd":"{repo}","worktree_path":"{wt}"}}"#,
        repo = repo.path().display(),
        wt = wt_path.display()
    );

    let out = run_remove_with_stdin(repo.path(), &payload);
    assert!(
        out.status.success(),
        "_claude-worktree-remove should exit 0 when hook succeeds; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        marker.exists(),
        "pre_rm hook should have created the marker file at {}",
        marker.display()
    );
}

/// When pre_rm hook fails, `_claude-worktree-remove` still exits 0 (advisory).
#[cfg(unix)]
#[test]
fn remove_pre_rm_hook_failure_is_advisory_exits_zero() {
    let repo = TestRepo::new();
    let wt_path = repo.create_worktree("claude-remove-hook-fail");

    std::fs::write(
        repo.path().join(".cwconfig.json"),
        r#"{"hooks":{"pre_rm":"exit 99"}}"#,
    )
    .unwrap();

    let payload = format!(
        r#"{{"hook_event_name":"WorktreeRemove","cwd":"{repo}","worktree_path":"{wt}"}}"#,
        repo = repo.path().display(),
        wt = wt_path.display()
    );

    let out = run_remove_with_stdin(repo.path(), &payload);
    assert!(
        out.status.success(),
        "advisory pre_rm failure must not cause non-zero exit; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    // A warning should appear on stderr.
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("pre_rm hook failed") || stderr.contains("advisory"),
        "stderr should contain a warning about the hook failure, got: {:?}",
        stderr
    );
}

// ===========================================================================
// _claude-worktree-remove — error handling
// ===========================================================================

/// Malformed JSON → non-zero exit.
#[test]
fn remove_invalid_json_exits_nonzero() {
    let repo = TestRepo::new();
    let out = run_remove_with_stdin(repo.path(), "{bad json");

    assert!(
        !out.status.success(),
        "invalid JSON should cause non-zero exit"
    );
}
