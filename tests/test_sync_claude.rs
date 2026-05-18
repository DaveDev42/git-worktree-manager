//! Integration tests for `gw sync-claude`.
//!
//! Verifies that `.claude/settings.json` is created/updated idempotently,
//! existing user hooks are preserved, and malformed JSON is rejected.

mod common;
use common::TestRepo;
use std::path::Path;
use std::process::Command;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Run `gw sync-claude` inside `cwd`, injecting `CW_SPAWN_AI_BIN` so the
/// command string is deterministic. Returns the process output.
fn run_sync_claude(cwd: &Path, bin: &str) -> std::process::Output {
    Command::new(TestRepo::cw_bin())
        .arg("sync-claude")
        .current_dir(cwd)
        .env("CW_SPAWN_AI_BIN", bin)
        .output()
        .expect("failed to spawn gw sync-claude")
}

/// Read and parse the settings.json file at `path`.
fn read_settings(path: &Path) -> serde_json::Value {
    let raw = std::fs::read_to_string(path).expect("settings.json must exist");
    serde_json::from_str(&raw).expect("valid JSON")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Running `gw sync-claude` in a fresh repo (no `.claude/` dir) creates
/// `.claude/settings.json` with all three hooks.
#[test]
fn creates_settings_with_three_hooks_from_scratch() {
    let repo = TestRepo::new();
    let cwd = repo.path();
    let fake_bin = "/usr/local/bin/gw";

    let out = run_sync_claude(cwd, fake_bin);
    assert!(
        out.status.success(),
        "gw sync-claude failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let settings_path = cwd.join(".claude").join("settings.json");
    assert!(
        settings_path.exists(),
        ".claude/settings.json must be created"
    );

    let v = read_settings(&settings_path);

    // PreToolUse with Bash matcher + guard command
    let pre = v["hooks"]["PreToolUse"]
        .as_array()
        .expect("PreToolUse array");
    assert_eq!(pre.len(), 1);
    assert_eq!(pre[0]["matcher"], "Bash");
    let guard_cmd = pre[0]["hooks"][0]["command"].as_str().expect("command");
    assert!(guard_cmd.contains(fake_bin));
    assert!(guard_cmd.ends_with(" guard --tool-input -"));

    // WorktreeCreate
    let create = v["hooks"]["WorktreeCreate"]
        .as_array()
        .expect("WorktreeCreate array");
    assert_eq!(create.len(), 1);
    let create_cmd = create[0]["hooks"][0]["command"].as_str().expect("command");
    assert!(create_cmd.ends_with(" _claude-worktree-create"));

    // WorktreeRemove
    let remove = v["hooks"]["WorktreeRemove"]
        .as_array()
        .expect("WorktreeRemove array");
    assert_eq!(remove.len(), 1);
    let remove_cmd = remove[0]["hooks"][0]["command"].as_str().expect("command");
    assert!(remove_cmd.ends_with(" _claude-worktree-remove"));
}

/// Running `gw sync-claude` twice does not duplicate entries.
#[test]
fn idempotent_two_runs_no_duplicates() {
    let repo = TestRepo::new();
    let cwd = repo.path();
    let fake_bin = "/usr/local/bin/gw";

    run_sync_claude(cwd, fake_bin);
    let out2 = run_sync_claude(cwd, fake_bin);
    assert!(
        out2.status.success(),
        "second run failed: {}",
        String::from_utf8_lossy(&out2.stderr)
    );

    let settings_path = cwd.join(".claude").join("settings.json");
    let v = read_settings(&settings_path);

    // Each hook type must still have exactly one entry.
    assert_eq!(
        v["hooks"]["PreToolUse"].as_array().unwrap().len(),
        1,
        "PreToolUse must have exactly 1 entry after 2 runs"
    );
    assert_eq!(
        v["hooks"]["WorktreeCreate"].as_array().unwrap().len(),
        1,
        "WorktreeCreate must have exactly 1 entry after 2 runs"
    );
    assert_eq!(
        v["hooks"]["WorktreeRemove"].as_array().unwrap().len(),
        1,
        "WorktreeRemove must have exactly 1 entry after 2 runs"
    );
}

/// An existing user PreToolUse hook with matcher=Write is preserved; only the
/// Bash entry is added.
#[test]
fn preserves_existing_user_pretooluse_write_hook() {
    let repo = TestRepo::new();
    let cwd = repo.path();
    let fake_bin = "/usr/local/bin/gw";

    // Pre-populate .claude/settings.json with a user hook.
    let claude_dir = cwd.join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    let settings_path = claude_dir.join("settings.json");
    std::fs::write(
        &settings_path,
        r#"{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Write",
        "hooks": [
          { "type": "command", "command": "/usr/local/bin/my-linter" }
        ]
      }
    ]
  }
}
"#,
    )
    .unwrap();

    run_sync_claude(cwd, fake_bin);
    let v = read_settings(&settings_path);

    let arr = v["hooks"]["PreToolUse"].as_array().unwrap();
    assert_eq!(arr.len(), 2, "Write hook + Bash hook = 2");

    let matchers: Vec<&str> = arr
        .iter()
        .filter_map(|e| e.get("matcher").and_then(|m| m.as_str()))
        .collect();
    assert!(matchers.contains(&"Write"), "Write hook must be preserved");
    assert!(matchers.contains(&"Bash"), "Bash hook must be added");
}

/// A Bash-matcher PreToolUse hook with a *different* command is preserved;
/// our guard entry is appended as a separate item.
#[test]
fn preserves_other_bash_hook_and_appends_ours() {
    let repo = TestRepo::new();
    let cwd = repo.path();
    let fake_bin = "/usr/local/bin/gw";

    let claude_dir = cwd.join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    let settings_path = claude_dir.join("settings.json");
    std::fs::write(
        &settings_path,
        r#"{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          { "type": "command", "command": "/usr/local/bin/other-guard" }
        ]
      }
    ]
  }
}
"#,
    )
    .unwrap();

    run_sync_claude(cwd, fake_bin);
    let v = read_settings(&settings_path);

    let arr = v["hooks"]["PreToolUse"].as_array().unwrap();
    assert_eq!(arr.len(), 2, "other-guard + ours = 2");

    // The user's entry must still be there.
    let other_present = arr.iter().any(|e| {
        e["hooks"][0]["command"]
            .as_str()
            .map(|c| c.contains("other-guard"))
            .unwrap_or(false)
    });
    assert!(other_present, "user's other-guard hook must be preserved");

    // Our guard must be there too.
    let ours_present = arr.iter().any(|e| {
        e.get("matcher").and_then(|m| m.as_str()) == Some("Bash")
            && e["hooks"][0]["command"]
                .as_str()
                .map(|c| c.ends_with(" guard --tool-input -"))
                .unwrap_or(false)
    });
    assert!(ours_present, "gw guard hook must be appended");
}

/// Malformed JSON in settings.json causes a non-zero exit and no overwrite.
#[test]
fn malformed_json_errors_without_overwriting() {
    let repo = TestRepo::new();
    let cwd = repo.path();
    let fake_bin = "/usr/local/bin/gw";

    let claude_dir = cwd.join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    let settings_path = claude_dir.join("settings.json");
    let bad_content = "{ this is not valid json }}}";
    std::fs::write(&settings_path, bad_content).unwrap();

    let out = run_sync_claude(cwd, fake_bin);
    assert!(
        !out.status.success(),
        "should exit non-zero on malformed JSON"
    );

    // File must not be modified.
    let after = std::fs::read_to_string(&settings_path).unwrap();
    assert_eq!(after, bad_content, "malformed file must not be overwritten");
}
