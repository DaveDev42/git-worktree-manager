//! Shared test-only env-var serialisation for the `operations` modules.
//!
//! Several modules (`pr_cache`, `clean`) have unit tests that mutate the same
//! process-global env vars (`GW_TEST_GH_JSON`, `GW_TEST_GH_FAIL`,
//! `GW_TEST_CACHE_DIR`). If each module owned its own mutex, tests in
//! different modules could hold different locks simultaneously and race on
//! the env vars — which is exactly what showed up as a flaky CI failure of
//! `pr_cache::tests::fetch_parses_gh_json_from_env` and
//! `pr_cache::tests::load_or_fetch_bypasses_disk_when_no_cache_true` after
//! `clean::tests` started using the same env vars.
//!
//! Centralising the lock + restore guard here forces all such tests through
//! a single mutex, eliminating the cross-module race.

use std::sync::{Mutex, MutexGuard};

static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Acquire the shared env-var lock. Pair with `EnvGuard::capture` for
/// panic-safe restoration of the captured keys.
pub(crate) fn env_lock() -> MutexGuard<'static, ()> {
    ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Captures the current values of the given env keys on construction and
/// restores them on drop (panic-safe).
pub(crate) struct EnvGuard {
    saved: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl EnvGuard {
    pub(crate) fn capture(keys: &[&'static str]) -> Self {
        let saved = keys.iter().map(|k| (*k, std::env::var_os(k))).collect();
        Self { saved }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (k, v) in self.saved.drain(..) {
            match v {
                Some(val) => std::env::set_var(k, val),
                None => std::env::remove_var(k),
            }
        }
    }
}
