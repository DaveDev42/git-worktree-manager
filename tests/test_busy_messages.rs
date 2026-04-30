use git_worktree_manager::operations::busy::{BusyInfo, BusySource, BusyTier};
use git_worktree_manager::operations::busy_messages::{render_busy_block, render_refusal};
use std::path::PathBuf;

fn hard_claude(secs: u64) -> BusyInfo {
    BusyInfo {
        pid: 0,
        cmd: "claude (session abc123)".into(),
        cwd: PathBuf::from("/tmp/wt"),
        source: BusySource::ClaudeSession,
        tier: BusyTier::Hard,
        tty: None,
        started_secs_ago: Some(secs),
    }
}

fn soft_proc(pid: u32, cmd: &str, tty: bool, started: u64) -> BusyInfo {
    BusyInfo {
        pid,
        cmd: cmd.into(),
        cwd: PathBuf::from("/tmp/wt"),
        source: BusySource::ProcessScan,
        tier: BusyTier::Soft,
        tty: Some(tty),
        started_secs_ago: Some(started),
    }
}

#[test]
fn soft_only_uses_warning_tone() {
    let s = render_refusal("feature-x", &[], &[soft_proc(123, "zsh", true, 60)]);
    assert!(
        s.contains("may be in use"),
        "soft-only must use 'may be in use' wording"
    );
    assert!(s.contains("Re-run with --force"));
    assert!(!s.contains("Cannot delete"));
}

#[test]
fn hard_only_uses_strong_refusal() {
    let s = render_refusal("feature-x", &[hard_claude(120)], &[]);
    assert!(s.contains("Cannot delete worktree 'feature-x'"));
    assert!(s.contains("Active Claude session"));
    assert!(s.contains("Use --force"));
}

#[test]
fn both_tiers_lead_with_hard_then_show_soft() {
    let s = render_refusal(
        "feature-x",
        &[hard_claude(60)],
        &[soft_proc(7, "cargo build", false, 30)],
    );
    let hard_pos = s.find("Active Claude session").unwrap();
    let soft_pos = s.find("Additional processes").unwrap();
    assert!(hard_pos < soft_pos, "Hard section must precede Soft");
}

// Tests for `render_busy_block` — the read-only variant used by
// `gw list`. Body sections (Active Claude session /
// Lockfile holder / cwd processes) are shared with `render_refusal` via
// `render_hard_section` / `render_soft_list`; only the header tone and
// the absence of `--force` guidance differ.

#[test]
fn busy_block_empty_inputs_yields_empty_string() {
    assert_eq!(render_busy_block("feature-x", &[], &[]), "");
}

#[test]
fn busy_block_uses_neutral_header_and_no_force_hint() {
    let s = render_busy_block("feature-x", &[hard_claude(120)], &[]);
    assert!(
        s.contains("Worktree 'feature-x' may be in use:"),
        "expected read-only header, got: {}",
        s
    );
    assert!(s.contains("Active Claude session"));
    assert!(
        !s.contains("--force"),
        "busy block must not mention --force; that's delete-specific"
    );
    assert!(
        !s.contains("Cannot delete"),
        "busy block must not use the delete-specific tone"
    );
}

#[test]
fn busy_block_renders_both_sections_when_both_tiers_present() {
    let s = render_busy_block(
        "feature-x",
        &[hard_claude(60)],
        &[soft_proc(7, "cargo build", false, 30)],
    );
    let hard_pos = s.find("Active Claude session").unwrap();
    let soft_pos = s.find("Processes with cwd in this worktree").unwrap();
    assert!(hard_pos < soft_pos, "Hard section must precede Soft");
    assert!(!s.contains("--force"));
}
