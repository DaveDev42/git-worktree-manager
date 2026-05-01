//! Integration tests for `gw spawn` / `spawn_in_worktree`.

mod common;
use common::TestRepo;

use std::sync::Mutex;

use git_worktree_manager::operations::ai_tools::spawn_in_worktree;

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
fn with_sentinel_ai<F: FnOnce()>(sentinel_script: &str, f: F) {
    let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    let guard = EnvGuard {
        saved: vec![
            ("CW_LAUNCH_METHOD", std::env::var_os("CW_LAUNCH_METHOD")),
            ("CW_AI_TOOL", std::env::var_os("CW_AI_TOOL")),
        ],
    };

    std::env::set_var("CW_LAUNCH_METHOD", "foreground");
    std::env::set_var("CW_AI_TOOL", sentinel_script);

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
        let result = spawn_in_worktree(&wt_path, None);
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

    // Script creates a sentinel and logs its argv (the prompt is appended by
    // get_ai_tool_merge_command when CW_AI_TOOL is set).
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
        let result = spawn_in_worktree(&wt_path, Some("hello"));
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

    // get_ai_tool_merge_command appends the prompt as the last argv element
    // when CW_AI_TOOL is set; verify the prompt arrived.
    let logged = std::fs::read_to_string(&argv_log).unwrap_or_default();
    assert!(
        logged.contains("hello"),
        "prompt 'hello' not found in argv log: {:?}",
        logged
    );
}
