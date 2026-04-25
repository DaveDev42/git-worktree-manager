use git_worktree_manager::operations::busy::{BusyInfo, BusySource, BusyTier};
use git_worktree_manager::operations::busy_messages::render_refusal;
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
