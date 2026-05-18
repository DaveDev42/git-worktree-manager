/// Integration tests for `gw new --emit json`.
mod common;

use common::TestRepo;

fn worktree_path(repo: &TestRepo, branch: &str) -> std::path::PathBuf {
    repo.path().parent().unwrap().join(format!(
        "{}-{}",
        repo.path().file_name().unwrap().to_str().unwrap(),
        branch,
    ))
}

// ===========================================================================
// --emit json — stdout is exactly one line of valid JSON
// ===========================================================================

#[test]
fn test_emit_json_stdout_is_single_line_json() {
    let repo = TestRepo::new();
    let output = repo.cw(&["new", "emit-json-test", "--emit", "json"]);

    assert!(
        output.status.success(),
        "gw new --emit json failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(
        lines.len(),
        1,
        "stdout must be exactly one line, got {} lines: {:?}",
        lines.len(),
        stdout
    );

    let parsed: serde_json::Value =
        serde_json::from_str(lines[0]).expect("stdout must be valid JSON");
    assert!(parsed.is_object(), "JSON must be an object");
}

// ===========================================================================
// --emit json — worktree_path field is a real directory
// ===========================================================================

#[test]
fn test_emit_json_worktree_path_exists() {
    let repo = TestRepo::new();
    let output = repo.cw(&["new", "emit-path-test", "--emit", "json"]);
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();

    let wt_path_str = parsed["worktree_path"]
        .as_str()
        .expect("worktree_path must be a string");
    let wt_path = std::path::Path::new(wt_path_str);

    assert!(
        wt_path.is_absolute(),
        "worktree_path must be absolute, got: {}",
        wt_path_str
    );
    assert!(
        wt_path.is_dir(),
        "worktree_path must be an existing directory, got: {}",
        wt_path_str
    );
}

// ===========================================================================
// --emit json — branch and base fields
// ===========================================================================

#[test]
fn test_emit_json_branch_and_base_fields() {
    let repo = TestRepo::new();
    let output = repo.cw(&["new", "emit-fields-test", "--emit", "json"]);
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();

    let branch = parsed["branch"].as_str().expect("branch must be a string");
    assert_eq!(branch, "emit-fields-test");

    let base = parsed["base"].as_str().expect("base must be a string");
    // default branch is main in test repos
    assert_eq!(base, "main");
}

// ===========================================================================
// --emit json — stdout has no styled human output leaking into it
// ===========================================================================

#[test]
fn test_emit_json_no_styled_output_in_stdout() {
    let repo = TestRepo::new();
    let output = repo.cw(&["new", "emit-clean-stdout", "--emit", "json"]);
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Human-readable words from the styled println! calls must not appear
    // in stdout — they should be on stderr.
    assert!(
        !stdout.contains("Creating new worktree"),
        "styled output leaked into stdout: {}",
        stdout
    );
    assert!(
        !stdout.contains("Worktree created successfully"),
        "styled output leaked into stdout: {}",
        stdout
    );
}

// ===========================================================================
// --emit json — custom base branch reflected in JSON
// ===========================================================================

#[test]
fn test_emit_json_custom_base_reflected() {
    let repo = TestRepo::new();
    repo.create_branch("develop");

    let output = repo.cw(&[
        "new",
        "emit-base-test",
        "--emit",
        "json",
        "--base",
        "develop",
    ]);
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();

    assert_eq!(parsed["base"].as_str().unwrap(), "develop");
    assert_eq!(parsed["branch"].as_str().unwrap(), "emit-base-test");
}

// ===========================================================================
// --emit json — worktree_path matches default path convention
// ===========================================================================

#[test]
fn test_emit_json_path_matches_convention() {
    let repo = TestRepo::new();
    let output = repo.cw(&["new", "emit-convention", "--emit", "json"]);
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();

    let wt_path = std::path::PathBuf::from(parsed["worktree_path"].as_str().unwrap());
    // Expected default: ../<repo>-<branch>
    let expected = worktree_path(&repo, "emit-convention");

    // Canonicalize both to resolve symlinks (macOS /var -> /private/var)
    let wt_canon = wt_path.canonicalize().unwrap_or(wt_path);
    let exp_canon = expected.canonicalize().unwrap_or(expected);
    assert_eq!(wt_canon, exp_canon);
}

// ===========================================================================
// --emit text (default) — no JSON on stdout
// ===========================================================================

#[test]
fn test_emit_text_default_no_json() {
    let repo = TestRepo::new();
    let output = repo.cw(&["new", "emit-text-test", "-T", "skip"]);
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Default text mode: human output on stdout, no JSON
    assert!(
        stdout.contains("Creating new worktree") || stdout.contains("Worktree created"),
        "text mode must have human output on stdout, got: {}",
        stdout
    );
    // Must not be a JSON line
    assert!(
        serde_json::from_str::<serde_json::Value>(stdout.trim()).is_err(),
        "text mode stdout must not be valid JSON"
    );
}
