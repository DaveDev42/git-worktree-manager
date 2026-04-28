//! Render `gw delete` refusal messages for the 3-tier busy model.
//! Pure string formatting; no I/O. Kept separate from `busy.rs` so the
//! detection logic can be tested without locale/styling concerns.

use crate::operations::busy::{BusyInfo, BusySource};

fn fmt_age(secs: u64) -> String {
    if secs < 60 {
        format!("{}s ago", secs)
    } else if secs < 3600 {
        format!(
            "{} minute{} ago",
            secs / 60,
            if secs / 60 == 1 { "" } else { "s" }
        )
    } else {
        format!(
            "{} hour{} ago",
            secs / 3600,
            if secs / 3600 == 1 { "" } else { "s" }
        )
    }
}

fn render_hard_section(out: &mut String, hard: &[BusyInfo]) {
    for h in hard {
        match h.source {
            BusySource::ClaudeSession => {
                out.push_str("  Active Claude session\n");
                if let Some(secs) = h.started_secs_ago {
                    out.push_str(&format!("    last activity: {}\n", fmt_age(secs)));
                }
                // cmd carries "claude (session <id>)"
                if let Some(id_part) = h.cmd.strip_prefix("claude (session ") {
                    let id = id_part.trim_end_matches(')');
                    out.push_str(&format!("    session: {}\n", id));
                }
            }
            BusySource::Lockfile => {
                out.push_str(&format!("  Lockfile holder: PID {} ({})\n", h.pid, h.cmd));
            }
            BusySource::ProcessScan => {
                // Should not appear in hard tier; render defensively.
                out.push_str(&format!("  PID {}  {}\n", h.pid, h.cmd));
            }
        }
        out.push('\n');
    }
}

fn render_soft_list(out: &mut String, soft: &[BusyInfo]) {
    for s in soft {
        let tty_label = match s.tty {
            Some(true) => "(interactive)",
            Some(false) => "(no tty)",
            None => "",
        };
        let age_label = match s.started_secs_ago {
            Some(secs) if secs < 90 => format!(" (started {})", fmt_age(secs)),
            _ => String::new(),
        };
        out.push_str(&format!(
            "    PID {:>6}  {}  {}{}\n",
            s.pid, s.cmd, tty_label, age_label
        ));
    }
}

/// Render the busy-status block (header + body) for read-only callers
/// like `gw status` / `gw list`. Same body sections as `render_refusal`
/// (Active Claude session / Lockfile holder / cwd processes) but with a
/// neutral header and no `--force` guidance. Returns an empty string
/// when both vectors are empty.
pub fn render_busy_block(branch_display: &str, hard: &[BusyInfo], soft: &[BusyInfo]) -> String {
    let mut out = String::new();
    if hard.is_empty() && soft.is_empty() {
        return out;
    }
    out.push_str(&format!(
        "⚠ Worktree '{}' may be in use:\n\n",
        branch_display
    ));
    if !hard.is_empty() {
        render_hard_section(&mut out, hard);
    }
    if !soft.is_empty() {
        out.push_str("  Processes with cwd in this worktree:\n");
        render_soft_list(&mut out, soft);
    }
    out
}

/// Render the user-facing refusal text for `gw delete`. Empty inputs in
/// both vectors is a programming error (caller should not have refused)
/// but is rendered as an empty string for safety.
pub fn render_refusal(branch_display: &str, hard: &[BusyInfo], soft: &[BusyInfo]) -> String {
    let mut out = String::new();
    match (hard.is_empty(), soft.is_empty()) {
        (true, true) => return out,
        (true, false) => {
            out.push_str(&format!(
                "⚠ Worktree '{}' may be in use:\n\n",
                branch_display
            ));
            out.push_str("  Processes with cwd in this worktree:\n");
            render_soft_list(&mut out, soft);
            out.push('\n');
            out.push_str("  These may malfunction if the worktree is deleted.\n");
            out.push_str("  Re-run with --force to delete anyway.\n");
        }
        (false, true) => {
            out.push_str(&format!(
                "✗ Cannot delete worktree '{}' — in use:\n\n",
                branch_display
            ));
            render_hard_section(&mut out, hard);
            out.push_str("  Use --force to delete anyway.\n");
        }
        (false, false) => {
            out.push_str(&format!(
                "✗ Cannot delete worktree '{}' — in use:\n\n",
                branch_display
            ));
            render_hard_section(&mut out, hard);
            out.push_str("  Additional processes with cwd in this worktree:\n");
            render_soft_list(&mut out, soft);
            out.push('\n');
            out.push_str("  Use --force to delete anyway.\n");
        }
    }
    out
}
