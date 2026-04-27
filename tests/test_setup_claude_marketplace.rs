//! End-to-end install layout tests for the local-marketplace approach.
//! `claude` CLI calls are stubbed via `setup_claude_with_cli` so tests
//! never spawn external processes.

use git_worktree_manager::operations::setup_claude::claude_cli::{ClaudeCli, ClaudeCliError};
use git_worktree_manager::operations::setup_claude::{paths, setup_claude_with_cli};
use std::cell::RefCell;
use std::fs;
use std::path::Path;

struct RecordingCli {
    calls: RefCell<Vec<String>>,
    available: bool,
}

impl RecordingCli {
    fn new(available: bool) -> Self {
        Self {
            calls: RefCell::new(Vec::new()),
            available,
        }
    }
    fn calls(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }
}

impl ClaudeCli for RecordingCli {
    fn is_available(&self) -> bool {
        self.available
    }
    fn marketplace_add(&self, path: &Path) -> Result<(), ClaudeCliError> {
        self.calls
            .borrow_mut()
            .push(format!("add:{}", path.display()));
        Ok(())
    }
    fn marketplace_update(&self, name: &str) -> Result<(), ClaudeCliError> {
        self.calls.borrow_mut().push(format!("mp-update:{}", name));
        Ok(())
    }
    fn plugin_install(&self, slug: &str) -> Result<(), ClaudeCliError> {
        self.calls.borrow_mut().push(format!("install:{}", slug));
        Ok(())
    }
    fn plugin_update(&self, slug: &str) -> Result<(), ClaudeCliError> {
        self.calls.borrow_mut().push(format!("update:{}", slug));
        Ok(())
    }
}

#[test]
fn install_writes_full_marketplace_tree() {
    let home = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    let cli = RecordingCli::new(true);

    setup_claude_with_cli(home.path(), data.path(), &cli).unwrap();

    let dl = data.path();
    assert!(
        paths::marketplace_manifest_under(dl).exists(),
        "marketplace.json"
    );
    assert!(paths::plugin_manifest_under(dl).exists(), "plugin.json");
    assert!(paths::command_gw_under(dl).exists(), "commands/gw.md");
    assert!(
        paths::skill_delegate_under(dl).exists(),
        "delegate SKILL.md"
    );
    assert!(paths::skill_manage_under(dl).exists(), "manage SKILL.md");
    assert!(
        paths::skill_manage_reference_under(dl).exists(),
        "manage references file"
    );
    assert!(paths::sentinel_under(dl).exists(), "sentinel marker");
}

#[test]
fn install_invokes_claude_cli_in_correct_order() {
    let home = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    let cli = RecordingCli::new(true);

    setup_claude_with_cli(home.path(), data.path(), &cli).unwrap();

    let calls = cli.calls();
    assert_eq!(calls.len(), 2, "fresh install: add + install");
    assert!(
        calls[0].starts_with("add:"),
        "first call must be marketplace add"
    );
    assert_eq!(
        calls[1], "install:gw@gw-local",
        "second must install plugin"
    );
}

#[test]
fn second_install_uses_update_path() {
    let home = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    let cli1 = RecordingCli::new(true);
    setup_claude_with_cli(home.path(), data.path(), &cli1).unwrap();

    // Simulate Claude Code having registered the plugin after the first run.
    // The new logic reads `<home>/.claude/plugins/installed_plugins.json`
    // as the source of truth for "is the plugin registered with Claude Code?"
    // Without this file, `claude_has_plugin_registered` returns false and the
    // second run would correctly fall back to the fresh add+install path.
    let plugins_dir = home.path().join(".claude").join("plugins");
    fs::create_dir_all(&plugins_dir).unwrap();
    fs::write(
        plugins_dir.join("installed_plugins.json"),
        br#"{"plugins":{"gw@gw-local":[{"scope":"user"}]}}"#,
    )
    .unwrap();

    let cli2 = RecordingCli::new(true);
    setup_claude_with_cli(home.path(), data.path(), &cli2).unwrap();

    let calls = cli2.calls();
    assert!(
        calls.iter().any(|c| c == "mp-update:gw-local"),
        "second run should call marketplace update; calls={:?}",
        calls
    );
    assert!(
        calls.iter().any(|c| c == "update:gw@gw-local"),
        "second run should call plugin update; calls={:?}",
        calls
    );
    assert!(
        !calls.iter().any(|c| c.starts_with("add:")),
        "second run must not re-add marketplace"
    );
}

/// Regression test for the "sentinel-but-not-registered" bug.
///
/// Sequence:
///   1. First `setup_claude_with_cli` → installs, sentinel created.
///   2. User runs `claude plugin uninstall gw@gw-local` →
///      `installed_plugins.json` no longer has our entry (simulated here by
///      simply not writing the file).
///   3. Second `setup_claude_with_cli` → must take the fresh add+install
///      path, NOT the update path, because Claude Code has no record of us.
///
/// Before the fix the old code branched on sentinel existence, so step 3
/// silently called update+update against a non-registered plugin — a no-op
/// that left the plugin absent.
#[test]
fn reinstall_after_claude_uninstall_uses_add_path() {
    let home = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();

    // First run: fresh install. Sentinel is created; no installed_plugins.json written.
    let cli1 = RecordingCli::new(true);
    setup_claude_with_cli(home.path(), data.path(), &cli1).unwrap();
    assert!(
        paths::sentinel_under(data.path()).exists(),
        "sentinel must exist after first install"
    );

    // Do NOT write installed_plugins.json — this simulates the user having
    // run `claude plugin uninstall gw@gw-local` after the first install,
    // which removes our entry from Claude Code's plugin registry while
    // leaving the sentinel file intact.

    // Second run: should detect "not registered in Claude Code" and re-run
    // the full add+install path, not the update path.
    let cli2 = RecordingCli::new(true);
    setup_claude_with_cli(home.path(), data.path(), &cli2).unwrap();

    let calls = cli2.calls();
    assert!(
        calls.iter().any(|c| c.starts_with("add:")),
        "re-install after uninstall must call marketplace add; calls={:?}",
        calls
    );
    assert!(
        calls.iter().any(|c| c == "install:gw@gw-local"),
        "re-install after uninstall must call plugin install; calls={:?}",
        calls
    );
    assert!(
        !calls.iter().any(|c| c == "mp-update:gw-local"),
        "re-install after uninstall must NOT call marketplace update; calls={:?}",
        calls
    );
    assert!(
        !calls.iter().any(|c| c == "update:gw@gw-local"),
        "re-install after uninstall must NOT call plugin update; calls={:?}",
        calls
    );
}

#[test]
fn install_succeeds_when_claude_cli_missing() {
    let home = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    let cli = RecordingCli::new(false);

    setup_claude_with_cli(home.path(), data.path(), &cli).unwrap();

    assert!(paths::marketplace_manifest_under(data.path()).exists());
    assert_eq!(
        cli.calls().len(),
        0,
        "no CLI calls when claude is not available"
    );
}

#[test]
fn install_removes_legacy_layouts() {
    let home = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    let legacy_skill = home.path().join(".claude/skills/gw");
    let legacy_plugin = home.path().join(".claude/plugins/gw");
    fs::create_dir_all(&legacy_skill).unwrap();
    fs::create_dir_all(&legacy_plugin).unwrap();
    fs::write(legacy_skill.join("SKILL.md"), b"x").unwrap();
    fs::write(legacy_plugin.join("plugin.json"), b"{}").unwrap();

    let cli = RecordingCli::new(true);
    setup_claude_with_cli(home.path(), data.path(), &cli).unwrap();

    assert!(!legacy_skill.exists());
    assert!(!legacy_plugin.exists());
}

#[test]
fn install_is_content_idempotent() {
    let home = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    let cli = RecordingCli::new(true);
    setup_claude_with_cli(home.path(), data.path(), &cli).unwrap();

    let mp = paths::marketplace_manifest_under(data.path());
    let mtime_1 = fs::metadata(&mp).unwrap().modified().unwrap();

    std::thread::sleep(std::time::Duration::from_millis(20));
    let cli2 = RecordingCli::new(true);
    setup_claude_with_cli(home.path(), data.path(), &cli2).unwrap();
    let mtime_2 = fs::metadata(&mp).unwrap().modified().unwrap();

    assert_eq!(
        mtime_1, mtime_2,
        "second install must not rewrite marketplace.json"
    );
}
