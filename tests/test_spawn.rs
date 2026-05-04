//! Integration tests for `gw spawn` / `spawn_in_worktree`.
//!
//! Unix-only: the AI tool stub is a shell script invoked via `bash -lc`, and
//! we mark it executable with `PermissionsExt::from_mode`. The Windows
//! foreground launcher uses `cmd /C`, which is a separate code path not
//! exercised here.

#![cfg(unix)]

mod common;
use common::TestRepo;

use std::sync::Mutex;

use git_worktree_manager::operations::ai_tools::{LaunchOptions, spawn_in_worktree};

/// Mutex to serialize env-var mutations so parallel test threads don't stomp
/// on each other's CW_AI_TOOL / CW_LAUNCH_METHOD values.
static ENV_MUTEX: Mutex<()> = Mutex::new(());

/// RAII guard that restores env vars on drop, even if the closure panics.
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

/// Execute `f` with `CW_LAUNCH_METHOD=foreground` and the AI tool set to a
/// script that creates `sentinel` then exits 0.  Restores env vars via Drop
/// so panic in the closure can't leave env vars in a dirty state.
///
/// Also prepends the cargo-built `gw` binary's directory to `PATH` so the
/// foreground launcher's `bash -lc "gw _spawn-ai …"` can resolve `gw`. CI
/// runners don't have `gw` on PATH from a prior `cargo install`, so without
/// this the spawn pipeline can't re-enter `gw` to read the materialized spec.
fn with_sentinel_ai<F: FnOnce()>(sentinel_script: &str, f: F) {
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
    std::env::set_var("CW_AI_TOOL", sentinel_script);

    // The library now emits `<current_exe> _spawn-ai …` rather than `gw …`,
    // so PATH augmentation alone won't help: `current_exe()` for this test
    // process is `target/debug/deps/test_spawn-…`, which has no `_spawn-ai`
    // subcommand. Point the spawn line at the cargo-built `gw` binary.
    let gw_bin = std::path::PathBuf::from(env!("CARGO_BIN_EXE_gw"));
    std::env::set_var("CW_SPAWN_AI_BIN", &gw_bin);
    if let Some(bin_dir) = gw_bin.parent() {
        let mut paths: Vec<std::path::PathBuf> = vec![bin_dir.to_path_buf()];
        if let Some(existing) = std::env::var_os("PATH") {
            paths.extend(std::env::split_paths(&existing));
        }
        if let Ok(joined) = std::env::join_paths(paths) {
            std::env::set_var("PATH", joined);
        }
    }

    f();

    drop(guard);
}

#[test]
fn spawn_in_worktree_launches_in_existing_worktree() {
    let repo = TestRepo::new();
    let wt_path = repo.create_worktree("feat-x");

    // Write a tiny executable script that creates a sentinel file.
    let script_path = wt_path.join("ai-tool.sh");
    let sentinel = wt_path.join(".spawn-ran");
    std::fs::write(
        &script_path,
        format!("#!/bin/sh\ntouch '{}'\nexit 0\n", sentinel.display()),
    )
    .expect("write ai-tool.sh");
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755))
            .expect("chmod ai-tool.sh");
    }

    with_sentinel_ai(&script_path.to_string_lossy(), || {
        let result = spawn_in_worktree(&wt_path, None, &LaunchOptions::from_term(None));
        assert!(
            result.is_ok(),
            "spawn_in_worktree returned Err: {:?}",
            result
        );
    });

    assert!(
        sentinel.exists(),
        "sentinel file not created — dispatch did not fire"
    );
}

#[test]
fn spawn_in_worktree_with_prompt() {
    let repo = TestRepo::new();
    let wt_path = repo.create_worktree("feat-y");

    // Script creates a sentinel and logs its argv. spawn_in_worktree appends
    // the prompt as the trailing positional arg of the interactive command.
    let script_path = wt_path.join("ai-tool.sh");
    let sentinel = wt_path.join(".spawn-ran");
    let argv_log = wt_path.join(".spawn-argv");
    std::fs::write(
        &script_path,
        format!(
            "#!/bin/sh\ntouch '{}'\nprintf '%s\\n' \"$@\" > '{}'\nexit 0\n",
            sentinel.display(),
            argv_log.display()
        ),
    )
    .expect("write ai-tool.sh");
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755))
            .expect("chmod ai-tool.sh");
    }

    with_sentinel_ai(&script_path.to_string_lossy(), || {
        let result = spawn_in_worktree(&wt_path, Some("hello"), &LaunchOptions::from_term(None));
        assert!(
            result.is_ok(),
            "spawn_in_worktree with prompt returned Err: {:?}",
            result
        );
    });

    assert!(
        sentinel.exists(),
        "sentinel file not created — dispatch did not fire"
    );

    // The prompt should arrive as the last argv element of an interactive
    // launch — no `--print` / `--non-interactive` flag injected.
    let logged = std::fs::read_to_string(&argv_log).unwrap_or_default();
    assert!(
        logged.contains("hello"),
        "prompt 'hello' not found in argv log: {:?}",
        logged
    );
    for forbidden in ["--print", "--tools=default", "--non-interactive"] {
        assert!(
            !logged.lines().any(|line| line == forbidden),
            "interactive launch must not inject {} flag; argv log: {:?}",
            forbidden,
            logged
        );
    }
}
