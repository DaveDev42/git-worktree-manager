//! Tests for `config::resolve_term_option`, which composes the CLI
//! `-T` override with the existing env / repo / global / default
//! resolution chain.
//!
//! Env mutation is serialized via a process-wide mutex (the same
//! pattern test_spawn.rs uses) because `cargo test` runs tests in
//! parallel threads of one process.

use git_worktree_manager::config::resolve_term_option;
use git_worktree_manager::constants::LaunchMethod;
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

fn with_env<F: FnOnce()>(vars: &[(&'static str, Option<&str>)], f: F) {
    let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let guard = EnvGuard {
        saved: vars
            .iter()
            .map(|(k, _)| (*k, std::env::var_os(k)))
            .collect(),
    };
    for (k, v) in vars {
        match v {
            Some(s) => std::env::set_var(k, s),
            None => std::env::remove_var(k),
        }
    }
    f();
    drop(guard);
}

#[test]
fn override_beats_env() {
    let cwd = tempfile::tempdir().expect("tempdir");
    with_env(&[("CW_LAUNCH_METHOD", Some("tmux"))], || {
        let (method, session) =
            resolve_term_option(Some("foreground"), cwd.path()).expect("resolve");
        assert_eq!(method, LaunchMethod::Foreground);
        assert!(session.is_none());
    });
}

#[test]
fn override_with_session_name() {
    let cwd = tempfile::tempdir().expect("tempdir");
    with_env(&[("CW_LAUNCH_METHOD", None)], || {
        let (method, session) =
            resolve_term_option(Some("tmux:mywork"), cwd.path()).expect("resolve");
        assert_eq!(method, LaunchMethod::Tmux);
        assert_eq!(session.as_deref(), Some("mywork"));
    });
}

#[test]
fn no_override_uses_env() {
    let cwd = tempfile::tempdir().expect("tempdir");
    with_env(&[("CW_LAUNCH_METHOD", Some("tmux"))], || {
        let (method, session) = resolve_term_option(None, cwd.path()).expect("resolve");
        assert_eq!(method, LaunchMethod::Tmux);
        assert!(session.is_none()); // env var doesn't carry session syntax
    });
}

#[test]
fn no_override_no_env_falls_to_default() {
    let cwd = tempfile::tempdir().expect("tempdir");
    // Use an isolated HOME so the real global config doesn't interfere.
    let fake_home = tempfile::tempdir().expect("fake_home tempdir");
    with_env(
        &[
            ("CW_LAUNCH_METHOD", None),
            ("HOME", Some(fake_home.path().to_str().expect("utf8 path"))),
        ],
        || {
            let (method, session) = resolve_term_option(None, cwd.path()).expect("resolve");
            assert_eq!(method, LaunchMethod::Foreground);
            assert!(session.is_none());
        },
    );
}

#[test]
fn override_unknown_method_errors() {
    let cwd = tempfile::tempdir().expect("tempdir");
    with_env(&[("CW_LAUNCH_METHOD", None)], || {
        let err = resolve_term_option(Some("not-a-launcher"), cwd.path())
            .expect_err("unknown method should error");
        let msg = err.to_string();
        assert!(
            msg.contains("not-a-launcher") || msg.to_lowercase().contains("invalid"),
            "expected error to mention the unknown method, got: {}",
            msg
        );
    });
}
