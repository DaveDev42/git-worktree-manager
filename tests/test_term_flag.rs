//! End-to-end integration tests for the -T/--term CLI flag.
//!
//! Uses the same env-mutex pattern as tests/test_spawn.rs: serializes
//! env mutation, restores via Drop.

#![cfg(unix)]

mod common;
use common::TestRepo;

use std::sync::Mutex;

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

fn with_clean_env<F: FnOnce()>(f: F) {
    let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let guard = EnvGuard {
        saved: vec![
            ("CW_LAUNCH_METHOD", std::env::var_os("CW_LAUNCH_METHOD")),
            ("CW_AI_TOOL", std::env::var_os("CW_AI_TOOL")),
        ],
    };
    std::env::remove_var("CW_LAUNCH_METHOD");
    std::env::remove_var("CW_AI_TOOL");
    f();
    drop(guard);
}

#[test]
fn new_dash_t_fg_overrides_env_and_runs_ai_in_worktree() {
    let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let repo = TestRepo::new();

    // Sentinel-script AI tool: writes a file in cwd then exits 0.
    let scratch = tempfile::tempdir().expect("scratch tempdir");
    let script_path = scratch.path().join("ai-tool.sh");
    let sentinel_name = "spawn-ran";
    std::fs::write(
        &script_path,
        format!("#!/bin/sh\ntouch \"$(pwd)/{}\"\nexit 0\n", sentinel_name),
    )
    .expect("write ai-tool.sh");
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755))
            .expect("chmod ai-tool.sh");
    }

    // Call the gw binary directly so we control CW_LAUNCH_METHOD on the
    // child explicitly. TestRepo::cw hardcodes CW_LAUNCH_METHOD=foreground,
    // which would mask the env-vs-CLI override we are trying to verify.
    let gw_bin = std::path::PathBuf::from(env!("CARGO_BIN_EXE_gw"));

    // Augment PATH so any inner `gw _spawn-ai` invocation resolves to our
    // built binary (mirrors with_sentinel_ai's PATH handling). The library
    // now emits `<current_exe> _spawn-ai`, so this child gw will spawn
    // itself by absolute path — but we keep the augment as belt-and-braces
    // for any indirect shell-out paths that don't go through current_exe.
    let mut path_dirs: Vec<std::path::PathBuf> =
        vec![gw_bin.parent().expect("gw bin has parent").to_path_buf()];
    if let Some(existing) = std::env::var_os("PATH") {
        path_dirs.extend(std::env::split_paths(&existing));
    }
    let new_path = std::env::join_paths(path_dirs).expect("join PATH");

    let out = std::process::Command::new(&gw_bin)
        .args(["new", "feat-tflag", "-T", "fg"])
        .current_dir(repo.path())
        .env("CW_LAUNCH_METHOD", "tmux")
        .env("CW_AI_TOOL", &script_path)
        .env("CW_SPAWN_AI_BIN", &gw_bin)
        .env("PATH", &new_path)
        .output()
        .expect("run gw new -T fg");

    assert!(
        out.status.success(),
        "gw new -T fg failed. stdout={:?} stderr={:?}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );

    let wt_path = repo.path().parent().expect("repo has parent").join(format!(
        "{}-feat-tflag",
        repo.path()
            .file_name()
            .expect("repo basename")
            .to_string_lossy()
    ));
    let sentinel = wt_path.join(sentinel_name);
    assert!(
        sentinel.exists(),
        "expected -T fg to invoke foreground launcher and run AI tool; sentinel {} missing. stdout={:?}",
        sentinel.display(),
        String::from_utf8_lossy(&out.stdout),
    );
}

#[test]
fn new_rejects_invalid_term_before_creating_worktree() {
    // Regression for #85: `gw new <branch> -T <bogus>` used to silently
    // succeed because the spawn_in_worktree call inside create_worktree was
    // wrapped in `let _ = …`. The worktree got created, the launch failed
    // silently, and the user was left with a phantom worktree. Now we
    // pre-flight `-T` at the entrypoint via parse_term_option.
    with_clean_env(|| {
        let repo = TestRepo::new();
        let out = repo.cw(&["new", "feat-bad-term", "-T", "does-not-exist"]);
        assert!(
            !out.status.success(),
            "expected non-zero exit for invalid -T value. stdout={:?} stderr={:?}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr),
        );
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains("Invalid launch method") || stderr.contains("does-not-exist"),
            "expected 'Invalid launch method' in stderr, got: {}",
            stderr
        );

        // The worktree must NOT have been created on disk. Also no branch.
        let wt_path = repo.path().parent().expect("repo has parent").join(format!(
            "{}-feat-bad-term",
            repo.path()
                .file_name()
                .expect("repo basename")
                .to_string_lossy()
        ));
        assert!(
            !wt_path.exists(),
            "phantom worktree at {} — pre-flight failed to short-circuit",
            wt_path.display()
        );

        let branches = repo.cw_stdout(&["_path", "--list-branches"]);
        assert!(
            !branches.contains("feat-bad-term"),
            "phantom branch 'feat-bad-term' present in: {}",
            branches
        );
    });
}

#[test]
fn new_rejects_term_with_no_term_at_cli() {
    with_clean_env(|| {
        let repo = TestRepo::new();
        let out = repo.cw(&["new", "feat-conflict", "-T", "fg", "--no-term"]);
        assert!(
            !out.status.success(),
            "expected non-zero exit for -T + --no-term. stdout={:?} stderr={:?}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr),
        );
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains("--no-term")
                || stderr.contains("--term")
                || stderr.contains("conflict"),
            "expected conflict message in stderr, got: {}",
            stderr
        );
    });
}
