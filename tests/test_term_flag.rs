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
