/// Terminal launcher implementations.
pub mod detached;
pub mod foreground;
pub mod iterm;
pub mod tmux;
pub mod wezterm;
pub mod zellij;

/// Wrap a command line so that, after it exits, the pane/window/tab keeps a
/// fresh interactive login shell instead of closing.
///
/// The tmux and zellij launchers (session, window/tab, panes) run the command
/// *as* the pane's program (`bash -lc <cmd>`), so when the AI tool exits the
/// pane/tab/session closes with it — you lose the worktree context and can't
/// run follow-up commands. WezTerm and iTerm don't have this problem because
/// they type the command into an already-running shell; tmux's own session
/// launcher sidesteps it by `send-keys`-ing into a pre-spawned shell.
///
/// Appending `; exec "${SHELL:-bash}" -l` makes the affected launchers behave
/// the same way: the command runs, then — regardless of its exit code (`;`,
/// not `&&`) — control drops to a fresh login shell in the same cwd. A
/// non-zero exit leaves the user at a prompt with the context intact rather
/// than closing the pane.
///
/// `${SHELL:-bash}` honors the user's login shell when the env var is present
/// (the common case when `gw` is run from an interactive session) and falls
/// back to `bash` otherwise. The result is still meant to be passed to
/// `bash -lc`, so `<cmd>` keeps whatever quoting it already carries.
pub fn keep_shell_after(command: &str) -> String {
    format!("{command}; exec \"${{SHELL:-bash}}\" -l")
}

#[cfg(test)]
mod tests {
    use super::keep_shell_after;

    #[test]
    fn appends_exec_login_shell() {
        assert_eq!(
            keep_shell_after("/usr/local/bin/gw _spawn-ai /tmp/x.json"),
            "/usr/local/bin/gw _spawn-ai /tmp/x.json; exec \"${SHELL:-bash}\" -l"
        );
    }

    #[test]
    fn preserves_existing_quoting() {
        // spawn_spec emits quoted segments when the path has spaces; we must
        // not disturb them — the whole thing is still handed to `bash -lc`.
        let cmd = r#""/My App/gw" _spawn-ai "/tmp/x y.json""#;
        assert_eq!(
            keep_shell_after(cmd),
            r#""/My App/gw" _spawn-ai "/tmp/x y.json"; exec "${SHELL:-bash}" -l"#
        );
    }
}
