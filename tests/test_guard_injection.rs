//! End-to-end test for `--settings` guard injection.
//!
//! Strategy: drop a fake `claude` executable on PATH that dumps its argv,
//! call `spawn_in_worktree`, then assert the fake `claude` saw `--settings`
//! followed by a JSON payload with PreToolUse(Bash) registered against
//! `gw guard --tool-input -`.

#![cfg(unix)]

mod common;
use common::TestRepo;

use std::path::Path;
use std::sync::Mutex;

use git_worktree_manager::operations::ai_tools::spawn_in_worktree;

static ENV_MUTEX: Mutex<()> = Mutex::new(());

struct EnvGuard {
    saved: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (k, v) in self.saved.drain(..) {
            match v {
                Some(s) => std::env::set_var(k, s),
                None => std::env::remove_var(k),
            }
        }
    }
}

fn write_fake_claude(dir: &Path, argv_log: &Path) -> std::path::PathBuf {
    let path = dir.join("claude");
    std::fs::write(
        &path,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\nexit 0\n",
            argv_log.display()
        ),
    )
    .expect("write fake claude");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
        .expect("chmod fake claude");
    path
}

/// Run `body` with environment plumbing so spawning launches our fake
/// `claude` at `<bin_dir>/claude`.
fn with_fake_claude<F: FnOnce()>(bin_dir: &Path, body: F) {
    let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let guard = EnvGuard {
        saved: vec![
            ("CW_LAUNCH_METHOD", std::env::var_os("CW_LAUNCH_METHOD")),
            ("CW_AI_TOOL", std::env::var_os("CW_AI_TOOL")),
            ("CW_SPAWN_AI_BIN", std::env::var_os("CW_SPAWN_AI_BIN")),
            ("PATH", std::env::var_os("PATH")),
        ],
    };

    std::env::set_var("CW_LAUNCH_METHOD", "foreground");
    // First word must be "claude" so is_claude_tool_for_cwd() returns true.
    std::env::set_var("CW_AI_TOOL", "claude");

    let gw_bin = std::path::PathBuf::from(env!("CARGO_BIN_EXE_gw"));
    std::env::set_var("CW_SPAWN_AI_BIN", &gw_bin);

    // PATH: fake-claude dir first, then gw bin dir, then existing PATH.
    let mut paths: Vec<std::path::PathBuf> = vec![bin_dir.to_path_buf()];
    if let Some(d) = gw_bin.parent() {
        paths.push(d.to_path_buf());
    }
    if let Some(existing) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&existing));
    }
    if let Ok(joined) = std::env::join_paths(paths) {
        std::env::set_var("PATH", joined);
    }

    body();

    drop(guard);
}

#[test]
fn spawn_injects_settings_with_pretooluse_bash_hook() {
    let repo = TestRepo::new();
    let wt_path = repo.create_worktree("feat-inject");

    let bin_dir = tempfile::tempdir().expect("tempdir");
    let argv_log = wt_path.join(".claude-argv");
    write_fake_claude(bin_dir.path(), &argv_log);

    with_fake_claude(bin_dir.path(), || {
        spawn_in_worktree(&wt_path, None, None).expect("spawn");
    });

    let dumped = std::fs::read_to_string(&argv_log).expect("argv log");
    let argv: Vec<&str> = dumped.lines().collect();
    let settings_pos = argv
        .iter()
        .position(|s| *s == "--settings")
        .unwrap_or_else(|| panic!("--settings not present in argv: {:?}", argv));
    let json_str = argv
        .get(settings_pos + 1)
        .expect("--settings must be followed by a JSON payload");

    let v: serde_json::Value = serde_json::from_str(json_str).expect("settings json parses");
    let arr = v["hooks"]["PreToolUse"]
        .as_array()
        .expect("PreToolUse array");
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["matcher"], "Bash");
    let cmd = arr[0]["hooks"][0]["command"].as_str().expect("command str");
    assert!(
        cmd.ends_with("guard --tool-input -"),
        "command should end with guard call, got: {cmd}"
    );
}

#[test]
fn spawn_omits_settings_when_guard_disabled_in_repo_config() {
    let repo = TestRepo::new();
    let wt_path = repo.create_worktree("feat-no-guard");

    // Disable guard via repo-local .cwconfig.json. Place it inside the
    // worktree so load_effective_config(cwd) walks up and finds it.
    std::fs::write(
        wt_path.join(".cwconfig.json"),
        r#"{"ai_tool": {"command": "claude", "args": [], "guard": false}}"#,
    )
    .expect("write .cwconfig.json");

    let bin_dir = tempfile::tempdir().expect("tempdir");
    let argv_log = wt_path.join(".claude-argv");
    write_fake_claude(bin_dir.path(), &argv_log);

    with_fake_claude(bin_dir.path(), || {
        spawn_in_worktree(&wt_path, None, None).expect("spawn");
    });

    let dumped = std::fs::read_to_string(&argv_log).expect("argv log");
    let argv: Vec<&str> = dumped.lines().collect();
    assert!(
        !argv.contains(&"--settings"),
        "guard=false should not inject --settings; argv={:?}",
        argv
    );
}
