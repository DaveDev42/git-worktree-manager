//! CLI-level tests for the `gw config` family.
//!
//! These run the real binary with `$HOME` pointed at a tempdir so they cannot
//! touch the developer's actual global config. Per-key parsing and TUI state
//! machine behavior are covered by unit tests adjacent to the implementations
//! in `src/operations/config_ops.rs` and `src/tui/config_editor.rs`; this
//! file covers what only shows up when the full binary is wired together —
//! most importantly, that read-only `gw config` subcommands do not have any
//! file-system side effects.

use assert_cmd::Command;
use predicates::prelude::*;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

/// Build a `gw` command with `$HOME` redirected to `home_dir`. We intentionally
/// do **not** preserve the caller's environment beyond what `assert_cmd` keeps
/// by default — the goal is to make sure nothing in `~/.config` leaks in or
/// out during the test.
fn gw(home_dir: &Path) -> Command {
    let mut c = Command::cargo_bin("gw").unwrap();
    c.env("HOME", home_dir);
    // On Windows, `dirs::home_dir()` reads `USERPROFILE` rather than `HOME`.
    // Set both so the production code lands inside the tempdir on every platform.
    c.env("USERPROFILE", home_dir);
    // `dirs::cache_dir()` respects `$XDG_CACHE_HOME` on Linux — used by the
    // auto-update checker (`update.rs`) and the PR cache. Without removal a
    // CI runner with `$XDG_CACHE_HOME` exported would write outside the
    // test's tempdir. `$XDG_CONFIG_HOME` is removed defensively: the
    // production code currently constructs `$HOME/.config/…` directly rather
    // than calling `dirs::config_dir()`, but anything reaching for the dirs
    // crate later should still land inside the tempdir.
    c.env_remove("XDG_CONFIG_HOME");
    c.env_remove("XDG_CACHE_HOME");
    c
}

fn global_config_path(home: &Path) -> PathBuf {
    home.join(".config")
        .join("git-worktree-manager")
        .join("config.json")
}

#[test]
fn config_list_does_not_create_global_file() {
    // The first invocation of any non-internal `gw` command used to run the
    // shell-completion hint, which persisted a default-filled JSON to the
    // global config path. That made every default row of `gw config list`
    // render as `[global]` instead of `[default]`. `gw config` is now skipped
    // by that hint; this test pins the new behavior.
    let home = tempdir().unwrap();
    let cwd = tempdir().unwrap(); // outside any git repo
    let home_path = home.path().to_path_buf();
    let cfg = global_config_path(&home_path);
    assert!(!cfg.exists(), "precondition: tempdir is empty");

    gw(&home_path)
        .current_dir(cwd.path())
        .args(["config", "list"])
        .assert()
        .success();

    assert!(
        !cfg.exists(),
        "`gw config list` must not create {} as a side effect",
        cfg.display()
    );
}

#[test]
fn config_get_does_not_create_global_file() {
    // Single-purpose: `get` on a key with a default returns success and does
    // not touch the global config file. The *value* it prints is asserted by
    // `config_get_prints_default_value` so changing a default here does not
    // mislead this regression guard.
    let home = tempdir().unwrap();
    let cwd = tempdir().unwrap();
    let home_path = home.path().to_path_buf();
    let cfg = global_config_path(&home_path);

    gw(&home_path)
        .current_dir(cwd.path())
        .args(["config", "get", "ai-tool.command"])
        .assert()
        .success();

    assert!(
        !cfg.exists(),
        "`gw config get` must not create {} as a side effect",
        cfg.display()
    );
}

#[test]
fn config_get_prints_default_value() {
    // Default-value assertion lives here, separate from the file-absence
    // guard above so a default change is a single localized failure.
    let home = tempdir().unwrap();
    let cwd = tempdir().unwrap();
    let home_path = home.path().to_path_buf();

    gw(&home_path)
        .current_dir(cwd.path())
        .args(["config", "get", "ai-tool.command"])
        .assert()
        .success()
        .stdout(predicate::str::contains("claude"));
}

#[test]
fn config_get_unset_key_exits_nonzero_with_no_output() {
    let home = tempdir().unwrap();
    let cwd = tempdir().unwrap();
    let home_path = home.path().to_path_buf();

    // `hooks.post-new` has no default; an unset key must exit non-zero with
    // no stdout (matches `git config`'s convention for script consumers).
    // `cwd` is outside any git repo so the repo-layer `.cwconfig.json` in
    // this project's own tree cannot leak into the test.
    gw(&home_path)
        .current_dir(cwd.path())
        .args(["config", "get", "hooks.post-new"])
        .assert()
        .failure()
        .stdout(predicate::str::is_empty());
}

#[test]
fn config_set_writes_global_file() {
    let home = tempdir().unwrap();
    let cwd = tempdir().unwrap();
    let home_path = home.path().to_path_buf();
    let cfg = global_config_path(&home_path);

    gw(&home_path)
        .current_dir(cwd.path())
        .args(["config", "set", "ai-tool.command", "codex"])
        .assert()
        .success();

    assert!(cfg.exists(), "set should have created the global file");
    let content = std::fs::read_to_string(&cfg).unwrap();
    assert!(content.contains("\"command\""));
    assert!(content.contains("\"codex\""));
}

#[test]
fn config_set_rejects_non_finite_number() {
    let home = tempdir().unwrap();
    let cwd = tempdir().unwrap();
    let home_path = home.path().to_path_buf();

    gw(&home_path)
        .current_dir(cwd.path())
        .args(["config", "set", "launch.wezterm-ready-timeout", "inf"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("finite"));
}

#[test]
fn config_set_repo_outside_repo_errors() {
    let home = tempdir().unwrap();
    let cwd = tempdir().unwrap(); // not a git repo
    let home_path = home.path().to_path_buf();

    gw(&home_path)
        .current_dir(cwd.path())
        .args(["config", "set", "--repo", "ai-tool.command", "codex"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("git repository"));
}

#[test]
fn config_edit_without_tty_errors_cleanly() {
    let home = tempdir().unwrap();
    let cwd = tempdir().unwrap();
    let home_path = home.path().to_path_buf();

    // stdout is a pipe in `assert_cmd` — `gw config edit` should detect that
    // and refuse with a non-zero exit + a clear message, instead of trying to
    // initialize ratatui and stranding the terminal.
    gw(&home_path)
        .current_dir(cwd.path())
        .args(["config", "edit"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("TTY"));
}
