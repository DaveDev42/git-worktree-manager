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
/// tmux windows/panes and zellij tabs/panes launch the command *as* the
/// pane's program (`bash -lc <cmd>`), so when the AI tool exits the pane
/// closes with it — you lose the worktree context and can't run follow-up
/// commands. WezTerm and iTerm don't have this problem because they type the
/// command into an already-running shell. Appending `exec "$SHELL"` makes the
/// multiplexer panes behave the same way: the command runs, then control
/// drops to a login shell in the same cwd.
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
