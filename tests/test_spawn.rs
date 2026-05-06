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

use git_worktree_manager::operations::ai_tools::{
    resume_worktree, spawn_in_worktree, LaunchOptions,
};

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

// ---------------------------------------------------------------------------
// End-to-end environment-forwarding tests.
//
// The IPC launchers (wezterm/iterm/tmux/zellij) inherit their daemon's env,
// not gw's parent shell's env, so the SpawnSpec.env map is the *only* path
// for `<TOOL>_*` vars to reach the child. Foreground execvp inherits parent
// env naturally, so checking child env directly conflates the two paths.
//
// Instead these tests inspect `gw-spawn-last.json` — the persistent copy
// of the SpawnSpec written to `<git-dir>/` before launch — which contains
// the exact env map every launcher reads from. This is the right boundary
// to test: it isolates "did gw snapshot the right vars" from "did the OS
// inherit them anyway."
//
// Setup quirks:
// - Sentinel script is named `claude` so `auto_forward_prefix` matches by
//   basename and returns "CLAUDE_". A generic `ai-tool.sh` would silently
//   produce no auto-forward and the test would pass for the wrong reason.
// - HOME is set to a fresh tempdir so `claude_native_session_exists`
//   doesn't observe a real session and flip to `--continue`.
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize)]
struct PersistedSpec {
    #[serde(default)]
    env: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    argv: Vec<String>,
}

fn write_claude_sentinel(dir: &std::path::Path) -> std::path::PathBuf {
    let script = dir.join("claude");
    std::fs::write(&script, "#!/bin/sh\nexit 0\n").expect("write claude sentinel");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
        .expect("chmod claude sentinel");
    script
}

/// Read the persisted spawn spec for a worktree.
/// Mirrors `gw _spawn-ai`'s recovery path: `<git-dir>/gw-spawn-last.json`.
fn last_spec(wt_path: &std::path::Path) -> PersistedSpec {
    let git_dir = std::process::Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .current_dir(wt_path)
        .output()
        .expect("git rev-parse");
    let raw = String::from_utf8_lossy(&git_dir.stdout).trim().to_string();
    let mut spec_path = std::path::PathBuf::from(&raw);
    if spec_path.is_relative() {
        spec_path = wt_path.join(spec_path);
    }
    spec_path.push("gw-spawn-last.json");
    let text = std::fs::read_to_string(&spec_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", spec_path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", spec_path.display()))
}

fn last_spec_env(wt_path: &std::path::Path) -> std::collections::BTreeMap<String, String> {
    last_spec(wt_path).env
}

#[test]
fn claude_prefixed_env_lands_in_spawn_spec() {
    let repo = TestRepo::new();
    let wt_path = repo.create_worktree("env-fwd-auto");
    let scratch = tempfile::tempdir().expect("scratch");
    let script = write_claude_sentinel(scratch.path());

    let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _guard = EnvGuard {
        saved: vec![
            ("CLAUDE_FOO_E2E", std::env::var_os("CLAUDE_FOO_E2E")),
            ("PLAIN_FOO_E2E", std::env::var_os("PLAIN_FOO_E2E")),
            ("HOME", std::env::var_os("HOME")),
            ("CW_LAUNCH_METHOD", std::env::var_os("CW_LAUNCH_METHOD")),
            ("CW_AI_TOOL", std::env::var_os("CW_AI_TOOL")),
        ],
    };
    let fake_home = tempfile::tempdir().expect("fake HOME");
    std::env::set_var("HOME", fake_home.path());
    std::env::set_var("CW_LAUNCH_METHOD", "foreground");
    std::env::set_var("CW_AI_TOOL", &script);
    std::env::set_var("CLAUDE_FOO_E2E", "auto-forwarded");
    std::env::set_var("PLAIN_FOO_E2E", "should-not-leak");

    let result = spawn_in_worktree(&wt_path, None, &LaunchOptions::from_term(None));
    assert!(result.is_ok(), "spawn returned Err: {:?}", result);

    let env = last_spec_env(&wt_path);
    assert_eq!(
        env.get("CLAUDE_FOO_E2E").map(String::as_str),
        Some("auto-forwarded"),
        "CLAUDE_FOO_E2E must be in the spawn spec env. spec env: {env:?}"
    );
    assert!(
        !env.contains_key("PLAIN_FOO_E2E"),
        "PLAIN_FOO_E2E (no `CLAUDE_` prefix) must not leak into spec env. spec env: {env:?}"
    );
}

#[test]
fn no_env_forward_yields_empty_spec_env() {
    let repo = TestRepo::new();
    let wt_path = repo.create_worktree("env-fwd-optout");
    let scratch = tempfile::tempdir().expect("scratch");
    let script = write_claude_sentinel(scratch.path());

    let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _guard = EnvGuard {
        saved: vec![
            ("CLAUDE_FOO_OPTOUT", std::env::var_os("CLAUDE_FOO_OPTOUT")),
            ("HOME", std::env::var_os("HOME")),
            ("CW_LAUNCH_METHOD", std::env::var_os("CW_LAUNCH_METHOD")),
            ("CW_AI_TOOL", std::env::var_os("CW_AI_TOOL")),
        ],
    };
    let fake_home = tempfile::tempdir().expect("fake HOME");
    std::env::set_var("HOME", fake_home.path());
    std::env::set_var("CW_LAUNCH_METHOD", "foreground");
    std::env::set_var("CW_AI_TOOL", &script);
    std::env::set_var("CLAUDE_FOO_OPTOUT", "should-be-suppressed");

    let opts = LaunchOptions {
        term_override: None,
        forward_args: &[],
        no_env_forward: true,
    };
    let result = spawn_in_worktree(&wt_path, None, &opts);
    assert!(
        result.is_ok(),
        "spawn with no_env_forward returned Err: {:?}",
        result
    );

    let env = last_spec_env(&wt_path);
    assert!(
        env.is_empty(),
        "no_env_forward must suppress all auto-forwarded vars from the spec. spec env: {env:?}"
    );
}

#[test]
fn unknown_ai_tool_yields_empty_spec_env() {
    // When the AI tool basename is not claude/codex/gemini, build_env_map
    // returns an empty map regardless of what's in the parent env.
    let repo = TestRepo::new();
    let wt_path = repo.create_worktree("env-fwd-unknown");
    let scratch = tempfile::tempdir().expect("scratch");
    let script = scratch.path().join("mytool");
    std::fs::write(&script, "#!/bin/sh\nexit 0\n").expect("write mytool");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
        .expect("chmod mytool");

    let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _guard = EnvGuard {
        saved: vec![
            ("CLAUDE_FOO_UNK", std::env::var_os("CLAUDE_FOO_UNK")),
            ("HOME", std::env::var_os("HOME")),
            ("CW_LAUNCH_METHOD", std::env::var_os("CW_LAUNCH_METHOD")),
            ("CW_AI_TOOL", std::env::var_os("CW_AI_TOOL")),
        ],
    };
    let fake_home = tempfile::tempdir().expect("fake HOME");
    std::env::set_var("HOME", fake_home.path());
    std::env::set_var("CW_LAUNCH_METHOD", "foreground");
    std::env::set_var("CW_AI_TOOL", &script);
    std::env::set_var("CLAUDE_FOO_UNK", "set-but-not-mytool-prefix");

    let result = spawn_in_worktree(&wt_path, None, &LaunchOptions::from_term(None));
    assert!(
        result.is_ok(),
        "spawn with unknown AI tool returned Err: {:?}",
        result
    );

    let env = last_spec_env(&wt_path);
    assert!(
        env.is_empty(),
        "unknown AI tool ('mytool') must not auto-forward anything. spec env: {env:?}"
    );
}

#[test]
fn forward_args_land_in_spawn_spec_argv() {
    // The user's trailing args after `--` must be forwarded verbatim to the
    // AI tool. The spawn spec's argv is the canonical record — the IPC
    // launchers pull argv from this map, not from anything reconstructed
    // later. So if the args land here in the right slot (after the AI
    // command, before any prompt), they make it to the child.
    let repo = TestRepo::new();
    let wt_path = repo.create_worktree("fwd-argv");
    let scratch = tempfile::tempdir().expect("scratch");
    let script = write_claude_sentinel(scratch.path());

    let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _guard = EnvGuard {
        saved: vec![
            ("HOME", std::env::var_os("HOME")),
            ("CW_LAUNCH_METHOD", std::env::var_os("CW_LAUNCH_METHOD")),
            ("CW_AI_TOOL", std::env::var_os("CW_AI_TOOL")),
            ("CW_SPAWN_AI_BIN", std::env::var_os("CW_SPAWN_AI_BIN")),
            ("PATH", std::env::var_os("PATH")),
        ],
    };
    let fake_home = tempfile::tempdir().expect("fake HOME");
    std::env::set_var("HOME", fake_home.path());
    std::env::set_var("CW_LAUNCH_METHOD", "foreground");
    std::env::set_var("CW_AI_TOOL", &script);
    // Point the spawn line at the cargo-built `gw` binary so the
    // foreground launcher's `bash -lc "<bin> _spawn-ai …"` resolves
    // cleanly instead of writing a confusing error to stderr.
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

    let forward = vec![
        "--model".to_string(),
        "opus".to_string(),
        "--append-system-prompt".to_string(),
        "be terse".to_string(),
    ];
    let opts = LaunchOptions {
        term_override: None,
        forward_args: &forward,
        no_env_forward: false,
    };
    let result = spawn_in_worktree(&wt_path, None, &opts);
    assert!(result.is_ok(), "spawn returned Err: {:?}", result);

    let argv = last_spec(&wt_path).argv;
    for needle in &forward {
        assert!(
            argv.iter().any(|a| a == needle),
            "forward arg {needle:?} missing from spec argv: {argv:?}"
        );
    }
    // The forward args must follow the AI command (first slot), and the
    // sequence of forward args must appear contiguously in the same order
    // the user passed them.
    let first = argv
        .iter()
        .position(|a| a == "--model")
        .expect("`--model` not found");
    assert_eq!(
        argv[first..first + forward.len()],
        forward[..],
        "forward args not contiguous in user-supplied order; argv: {argv:?}"
    );
}

#[test]
fn prompt_plus_forward_args_is_rejected() {
    // `--prompt` and trailing forward args both end up setting the AI
    // tool's prompt; combining them would silently produce two prompts.
    // The dispatcher must reject this before any side effects fire.
    let repo = TestRepo::new();
    let wt_path = repo.create_worktree("conflict");
    let scratch = tempfile::tempdir().expect("scratch");
    let script = write_claude_sentinel(scratch.path());

    let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _guard = EnvGuard {
        saved: vec![
            ("HOME", std::env::var_os("HOME")),
            ("CW_LAUNCH_METHOD", std::env::var_os("CW_LAUNCH_METHOD")),
            ("CW_AI_TOOL", std::env::var_os("CW_AI_TOOL")),
        ],
    };
    let fake_home = tempfile::tempdir().expect("fake HOME");
    std::env::set_var("HOME", fake_home.path());
    std::env::set_var("CW_LAUNCH_METHOD", "foreground");
    std::env::set_var("CW_AI_TOOL", &script);

    let forward = vec!["--model".to_string(), "opus".to_string()];
    let opts = LaunchOptions {
        term_override: None,
        forward_args: &forward,
        no_env_forward: false,
    };
    let result = spawn_in_worktree(&wt_path, Some("hi"), &opts);
    let err = result.expect_err("prompt + forward_args must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("--prompt") && msg.contains("AI tool args"),
        "error message should mention --prompt and forward args; got: {msg}"
    );
}

#[test]
fn resume_injects_continue_flag_even_without_local_session() {
    // README contract: `gw resume` always re-injects the AI tool's resume
    // flag (`--continue` for claude). The previous behaviour gated this on
    // `claude_native_session_exists`, which silently dropped the flag when
    // the local session-cache check failed (different HOME, fresh worktree,
    // unset CLAUDE_CONFIG_DIR, …) — even though the user explicitly ran
    // `gw resume`. The HOME tempdir below guarantees no native session is
    // detected, so this test exercises the formerly-broken path.
    let repo = TestRepo::new();
    let wt_path = repo.create_worktree("resume-no-session");
    let scratch = tempfile::tempdir().expect("scratch");
    let script = write_claude_sentinel(scratch.path());

    let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _guard = EnvGuard {
        saved: vec![
            ("HOME", std::env::var_os("HOME")),
            ("CW_LAUNCH_METHOD", std::env::var_os("CW_LAUNCH_METHOD")),
            ("CW_AI_TOOL", std::env::var_os("CW_AI_TOOL")),
            ("CW_SPAWN_AI_BIN", std::env::var_os("CW_SPAWN_AI_BIN")),
            ("PATH", std::env::var_os("PATH")),
        ],
    };
    // Empty HOME guarantees `claude_native_session_exists` returns false —
    // the regression path we're locking down.
    let fake_home = tempfile::tempdir().expect("fake HOME");
    std::env::set_var("HOME", fake_home.path());
    std::env::set_var("CW_LAUNCH_METHOD", "foreground");
    std::env::set_var("CW_AI_TOOL", &script);
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
    // Resolution of the no-target branch reads cwd, so chdir into the worktree.
    let prev_cwd = std::env::current_dir().expect("cwd");
    std::env::set_current_dir(&wt_path).expect("chdir into worktree");

    let result = resume_worktree(None, &LaunchOptions::default());
    let _ = std::env::set_current_dir(&prev_cwd);
    assert!(result.is_ok(), "resume returned Err: {:?}", result);

    // Either form is acceptable: the resume preset for `claude` injects
    // `--continue`, but when CW_AI_TOOL points at an arbitrary binary
    // (as it does here) the universal fallback in
    // `get_ai_tool_resume_command_for_cwd` appends `--resume` instead.
    // The user-facing contract is "some resume flag is always injected,"
    // not specifically `--continue`.
    let argv = last_spec(&wt_path).argv;
    assert!(
        argv.iter().any(|a| a == "--continue" || a == "--resume"),
        "`gw resume` against a worktree with no native session must still \
         inject a resume flag (README's 'always re-injects' contract). argv: {argv:?}"
    );
}

#[test]
fn resume_with_forward_args_keeps_continue_flag() {
    // Belt-and-suspenders: even when the user passes forward_args, the
    // resume flag must survive. forward_args should slot AFTER the preset
    // (which already contains --continue), not replace it.
    let repo = TestRepo::new();
    let wt_path = repo.create_worktree("resume-fwd");
    let scratch = tempfile::tempdir().expect("scratch");
    let script = write_claude_sentinel(scratch.path());

    let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _guard = EnvGuard {
        saved: vec![
            ("HOME", std::env::var_os("HOME")),
            ("CW_LAUNCH_METHOD", std::env::var_os("CW_LAUNCH_METHOD")),
            ("CW_AI_TOOL", std::env::var_os("CW_AI_TOOL")),
            ("CW_SPAWN_AI_BIN", std::env::var_os("CW_SPAWN_AI_BIN")),
            ("PATH", std::env::var_os("PATH")),
        ],
    };
    let fake_home = tempfile::tempdir().expect("fake HOME");
    std::env::set_var("HOME", fake_home.path());
    std::env::set_var("CW_LAUNCH_METHOD", "foreground");
    std::env::set_var("CW_AI_TOOL", &script);
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
    let prev_cwd = std::env::current_dir().expect("cwd");
    std::env::set_current_dir(&wt_path).expect("chdir into worktree");

    let forward = vec!["--model".to_string(), "haiku".to_string()];
    let opts = LaunchOptions {
        term_override: None,
        forward_args: &forward,
        no_env_forward: false,
    };
    let result = resume_worktree(None, &opts);
    let _ = std::env::set_current_dir(&prev_cwd);
    assert!(result.is_ok(), "resume returned Err: {:?}", result);

    let argv = last_spec(&wt_path).argv;
    assert!(
        argv.iter().any(|a| a == "--continue" || a == "--resume"),
        "resume flag must survive forward_args. argv: {argv:?}"
    );
    for needle in &forward {
        assert!(
            argv.iter().any(|a| a == needle),
            "forward arg {needle:?} missing. argv: {argv:?}"
        );
    }
}
