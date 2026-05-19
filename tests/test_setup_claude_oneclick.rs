//! Integration tests for `gw setup-claude` (one-click installer).
//!
//! Verifies that skill files and `.claude/settings.json` are created/updated
//! idempotently, existing user hooks are preserved, malformed JSON is rejected,
//! and that skill files are refreshed when content drifts.

mod common;
use common::TestRepo;
use std::path::Path;
use std::process::Command;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Run `gw setup-claude` inside `cwd`. Hook commands are emitted with a bare
/// `gw` name, so there's no per-machine path to inject — the output is
/// deterministic across hosts.
fn run_setup_claude(cwd: &Path) -> std::process::Output {
    Command::new(TestRepo::cw_bin())
        .arg("setup-claude")
        .current_dir(cwd)
        .output()
        .expect("failed to spawn gw setup-claude")
}

/// Read and parse the settings.json file at `path`.
fn read_settings(path: &Path) -> serde_json::Value {
    let raw = std::fs::read_to_string(path).expect("settings.json must exist");
    serde_json::from_str(&raw).expect("valid JSON")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Running `gw setup-claude` in a fresh repo creates all four files:
/// - `.claude/skills/gw-delegate/SKILL.md`
/// - `.claude/skills/gw-manage/SKILL.md`
/// - `.claude/skills/gw-manage/references/gw-commands.md`
/// - `.claude/settings.json` (with all three hooks)
#[test]
fn creates_all_files_from_scratch() {
    let repo = TestRepo::new();
    let cwd = repo.path();

    let out = run_setup_claude(cwd);
    assert!(
        out.status.success(),
        "gw setup-claude failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Skill files
    assert!(
        cwd.join(".claude/skills/gw-delegate/SKILL.md").exists(),
        "gw-delegate/SKILL.md must be created"
    );
    assert!(
        cwd.join(".claude/skills/gw-manage/SKILL.md").exists(),
        "gw-manage/SKILL.md must be created"
    );
    assert!(
        cwd.join(".claude/skills/gw-manage/references/gw-commands.md")
            .exists(),
        "gw-commands.md must be created"
    );

    // settings.json with three hooks, all using the bare `gw` form
    let settings_path = cwd.join(".claude/settings.json");
    assert!(
        settings_path.exists(),
        ".claude/settings.json must be created"
    );

    let v = read_settings(&settings_path);

    let pre = v["hooks"]["PreToolUse"]
        .as_array()
        .expect("PreToolUse array");
    assert_eq!(pre.len(), 1);
    assert_eq!(pre[0]["matcher"], "Bash");
    assert_eq!(
        pre[0]["hooks"][0]["command"].as_str().expect("command"),
        "gw guard --tool-input -"
    );

    let create = v["hooks"]["WorktreeCreate"]
        .as_array()
        .expect("WorktreeCreate array");
    assert_eq!(create.len(), 1);
    assert_eq!(
        create[0]["hooks"][0]["command"].as_str().expect("command"),
        "gw _claude-worktree-create"
    );

    let remove = v["hooks"]["WorktreeRemove"]
        .as_array()
        .expect("WorktreeRemove array");
    assert_eq!(remove.len(), 1);
    assert_eq!(
        remove[0]["hooks"][0]["command"].as_str().expect("command"),
        "gw _claude-worktree-remove"
    );
}

/// Running `gw setup-claude` twice does not duplicate entries or skill files.
#[test]
fn idempotent_two_runs() {
    let repo = TestRepo::new();
    let cwd = repo.path();

    run_setup_claude(cwd);
    let out2 = run_setup_claude(cwd);
    assert!(
        out2.status.success(),
        "second run failed: {}",
        String::from_utf8_lossy(&out2.stderr)
    );

    // Hook counts must still be exactly 1 each
    let settings_path = cwd.join(".claude/settings.json");
    let v = read_settings(&settings_path);
    assert_eq!(v["hooks"]["PreToolUse"].as_array().unwrap().len(), 1);
    assert_eq!(v["hooks"]["WorktreeCreate"].as_array().unwrap().len(), 1);
    assert_eq!(v["hooks"]["WorktreeRemove"].as_array().unwrap().len(), 1);

    // Skill files still present
    assert!(cwd.join(".claude/skills/gw-delegate/SKILL.md").exists());
    assert!(cwd.join(".claude/skills/gw-manage/SKILL.md").exists());
}

/// An existing user PreToolUse(Write) hook is preserved; only the Bash entry
/// is added alongside it.
#[test]
fn preserves_existing_user_pretooluse_write_hook() {
    let repo = TestRepo::new();
    let cwd = repo.path();

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

    let out = run_setup_claude(cwd);
    assert!(out.status.success());

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

/// Malformed JSON in settings.json causes a non-zero exit and no overwrite.
#[test]
fn malformed_json_errors_without_overwriting() {
    let repo = TestRepo::new();
    let cwd = repo.path();

    let claude_dir = cwd.join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    let settings_path = claude_dir.join("settings.json");
    let bad_content = "{ this is not valid json }}}";
    std::fs::write(&settings_path, bad_content).unwrap();

    let out = run_setup_claude(cwd);
    assert!(
        !out.status.success(),
        "should exit non-zero on malformed JSON"
    );

    let after = std::fs::read_to_string(&settings_path).unwrap();
    assert_eq!(after, bad_content, "malformed file must not be overwritten");
}

/// A manually modified SKILL.md is overwritten on re-run (content-addressed).
#[test]
fn refreshes_drifted_skill_file() {
    let repo = TestRepo::new();
    let cwd = repo.path();

    // First install
    run_setup_claude(cwd);

    // Manually corrupt the delegate skill
    let skill_path = cwd.join(".claude/skills/gw-delegate/SKILL.md");
    let original = std::fs::read_to_string(&skill_path).unwrap();
    let modified = format!("{original}\n# MANUALLY MODIFIED\n");
    std::fs::write(&skill_path, &modified).unwrap();

    // Re-run should restore it
    let out = run_setup_claude(cwd);
    assert!(out.status.success());

    let after = std::fs::read_to_string(&skill_path).unwrap();
    assert_eq!(
        after, original,
        "skill file must be restored to embedded content"
    );
}

/// Legacy absolute-path entries written by gw ≤ 0.1.11 are migrated to the
/// new bare-name form on the next `setup-claude` run. The user's unrelated
/// hooks are preserved.
#[test]
fn migrates_legacy_absolute_path_entries() {
    let repo = TestRepo::new();
    let cwd = repo.path();

    let claude_dir = cwd.join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    let settings_path = claude_dir.join("settings.json");

    // Hand-write a settings.json that mimics what 0.1.11 would have produced.
    std::fs::write(
        &settings_path,
        r#"{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          { "type": "command", "command": "/usr/local/bin/gw guard --tool-input -" }
        ]
      },
      {
        "matcher": "Write",
        "hooks": [
          { "type": "command", "command": "/usr/local/bin/my-linter" }
        ]
      }
    ],
    "WorktreeCreate": [
      {
        "hooks": [
          { "type": "command", "command": "/opt/homebrew/bin/gw _claude-worktree-create" }
        ]
      }
    ],
    "WorktreeRemove": [
      {
        "hooks": [
          { "type": "command", "command": "/Users/dave/proj/target/release/gw _claude-worktree-remove" }
        ]
      }
    ]
  }
}
"#,
    )
    .unwrap();

    let out = run_setup_claude(cwd);
    assert!(
        out.status.success(),
        "migration run failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let v = read_settings(&settings_path);

    // PreToolUse: user's Write hook preserved, gw's Bash hook migrated to
    // bare-name form, no duplicates.
    let pre = v["hooks"]["PreToolUse"].as_array().unwrap();
    assert_eq!(pre.len(), 2, "Write hook + migrated Bash hook");
    let pre_cmds: Vec<&str> = pre
        .iter()
        .map(|e| e["hooks"][0]["command"].as_str().unwrap())
        .collect();
    assert!(pre_cmds.contains(&"gw guard --tool-input -"));
    assert!(pre_cmds.contains(&"/usr/local/bin/my-linter"));

    let create = v["hooks"]["WorktreeCreate"].as_array().unwrap();
    assert_eq!(create.len(), 1);
    assert_eq!(
        create[0]["hooks"][0]["command"],
        "gw _claude-worktree-create"
    );

    let remove = v["hooks"]["WorktreeRemove"].as_array().unwrap();
    assert_eq!(remove.len(), 1);
    assert_eq!(
        remove[0]["hooks"][0]["command"],
        "gw _claude-worktree-remove"
    );
}
