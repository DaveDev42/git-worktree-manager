/// AI tool integration operations.
///
/// Handles launching AI coding assistants in various terminal environments.
use std::collections::BTreeMap;
use std::path::Path;

use console::style;

use crate::config::{
    self, get_ai_tool_command, get_ai_tool_resume_command, is_claude_tool, is_claude_tool_for_cwd,
    load_effective_config,
};
use crate::constants::{LaunchMethod, MAX_SESSION_NAME_LENGTH};
use crate::error::{CwError, Result};
use crate::git;
use crate::messages;
use crate::session;

use super::claude_settings;
use super::helpers::{resolve_target_strict, resolve_worktree_target};
use super::launchers;
use super::spawn_spec::{self, SpawnSpec};

/// Per-invocation knobs that ride alongside `term_override` and `prompt`
/// into every AI-tool launcher path. Bundled so a future option (extra env,
/// extra args, `--reason`-style metadata) doesn't fan out into every
/// signature.
#[derive(Debug, Default, Clone)]
pub struct LaunchOptions<'a> {
    /// `-T/--term` override.
    pub term_override: Option<&'a str>,
    /// Trailing args forwarded verbatim to the AI tool (after the preset's
    /// own args, before the prompt positional).
    pub forward_args: &'a [String],
    /// `--env KEY=VAL` entries (already validated).
    pub extra_env: &'a [(String, String)],
    /// True when `--no-env-forward` was passed.
    pub no_env_forward: bool,
}

impl<'a> LaunchOptions<'a> {
    /// Convenience constructor for callers that only have `-T`.
    pub fn from_term(term_override: Option<&'a str>) -> Self {
        Self {
            term_override,
            forward_args: &[],
            extra_env: &[],
            no_env_forward: false,
        }
    }
}

/// Dispatch a pre-materialized command to the configured launcher.
///
/// Both `launch_ai_tool` and `spawn_in_worktree` share this block; keeping it
/// in one place means any launcher added in the future is automatically
/// available to both callers.
fn dispatch_launch(
    path: &Path,
    method: LaunchMethod,
    session_name: Option<String>,
    cmd: &str,
    ai_tool_name: &str,
) -> Result<()> {
    match method {
        LaunchMethod::Skip => {
            // Should be intercepted by callers before dispatch (so they can
            // skip ai-tool resolution and the executable check entirely).
            // Treat as a defensive no-op if reached.
        }
        LaunchMethod::Foreground => {
            println!(
                "{}\n",
                style(messages::starting_ai_tool_foreground(ai_tool_name)).cyan()
            );
            // `_session_lock` binding is intentional: RAII guard lives for
            // the foreground AI process lifetime; dropped on return.
            let _session_lock = match crate::operations::lockfile::acquire(path, ai_tool_name) {
                Ok(lock) => Some(lock),
                Err(err @ crate::operations::lockfile::AcquireError::ForeignLock(_)) => {
                    return Err(crate::error::CwError::Other(format!(
                        "{}; exit that session first",
                        err
                    )));
                }
                Err(e) => {
                    eprintln!(
                        "{} could not write session lock: {}",
                        style("warning:").yellow(),
                        e
                    );
                    None
                }
            };
            launchers::foreground::run(path, cmd);
        }
        LaunchMethod::Detach => {
            launchers::detached::run(path, cmd);
            println!(
                "{} {} detached (survives terminal close)\n",
                style("*").green().bold(),
                ai_tool_name
            );
        }
        // iTerm
        LaunchMethod::ItermWindow => launchers::iterm::launch_window(path, cmd, ai_tool_name)?,
        LaunchMethod::ItermTab => launchers::iterm::launch_tab(path, cmd, ai_tool_name)?,
        LaunchMethod::ItermPaneH => launchers::iterm::launch_pane(path, cmd, ai_tool_name, true)?,
        LaunchMethod::ItermPaneV => launchers::iterm::launch_pane(path, cmd, ai_tool_name, false)?,
        // tmux
        LaunchMethod::Tmux => {
            let sn = session_name.unwrap_or_else(|| generate_session_name(path));
            launchers::tmux::launch_session(path, cmd, ai_tool_name, &sn)?;
        }
        LaunchMethod::TmuxWindow => launchers::tmux::launch_window(path, cmd, ai_tool_name)?,
        LaunchMethod::TmuxPaneH => launchers::tmux::launch_pane(path, cmd, ai_tool_name, true)?,
        LaunchMethod::TmuxPaneV => launchers::tmux::launch_pane(path, cmd, ai_tool_name, false)?,
        // Zellij
        LaunchMethod::Zellij => {
            let sn = session_name.unwrap_or_else(|| generate_session_name(path));
            launchers::zellij::launch_session(path, cmd, ai_tool_name, &sn)?;
        }
        LaunchMethod::ZellijTab => launchers::zellij::launch_tab(path, cmd, ai_tool_name)?,
        LaunchMethod::ZellijPaneH => launchers::zellij::launch_pane(path, cmd, ai_tool_name, true)?,
        LaunchMethod::ZellijPaneV => {
            launchers::zellij::launch_pane(path, cmd, ai_tool_name, false)?
        }
        // WezTerm
        LaunchMethod::WeztermWindow => launchers::wezterm::launch_window(path, cmd, ai_tool_name)?,
        LaunchMethod::WeztermTab => launchers::wezterm::launch_tab(path, cmd, ai_tool_name)?,
        LaunchMethod::WeztermTabBg => launchers::wezterm::launch_tab_bg(path, cmd, ai_tool_name)?,
        LaunchMethod::WeztermPaneH => {
            launchers::wezterm::launch_pane(path, cmd, ai_tool_name, true)?
        }
        LaunchMethod::WeztermPaneV => {
            launchers::wezterm::launch_pane(path, cmd, ai_tool_name, false)?
        }
    }

    Ok(())
}

/// Recognized parent-env prefix to auto-forward, keyed by the AI tool's
/// binary name. Returns `None` for tools we don't have a convention for —
/// in that case `--no-env-forward` becomes a no-op (nothing to forward).
fn auto_forward_prefix(ai_tool_name: &str) -> Option<&'static str> {
    match ai_tool_name {
        "claude" => Some("CLAUDE_"),
        "codex" => Some("CODEX_"),
        "gemini" => Some("GEMINI_"),
        _ => None,
    }
}

/// Build the env map injected into the spawned AI tool process.
///
/// Order of merging (later wins):
///   1. Auto-forwarded `<TOOL>_*` vars from the current (gw) process — the
///      shell that ran `gw` is the source of truth, so launchers like
///      wezterm/iterm/tmux/zellij (which spawn their own shells inside the
///      window-server's environment) still see the user's settings.
///   2. Caller-supplied `--env KEY=VAL` overrides.
///
/// Auto-forward is suppressed when `no_env_forward` is set or when
/// `auto_forward_prefix` returns `None` for this tool.
fn build_env_map(
    ai_tool_name: &str,
    extra_env: &[(String, String)],
    no_env_forward: bool,
) -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();

    if !no_env_forward {
        if let Some(prefix) = auto_forward_prefix(ai_tool_name) {
            for (k, v) in std::env::vars() {
                if k.starts_with(prefix) {
                    env.insert(k, v);
                }
            }
        }
    }

    for (k, v) in extra_env {
        env.insert(k.clone(), v.clone());
    }

    env
}

/// Parse a `--env KEY=VAL` token. KEY must be non-empty, and follow POSIX
/// portable env name rules (alphanumeric + underscore, not starting with a
/// digit). Returns `(KEY, VAL)` on success, or a CwError on a malformed entry.
///
/// Empty VAL is allowed (`--env FOO=`) — it intentionally clears any
/// auto-forwarded same-named var inside the spawned process.
pub fn parse_env_entry(raw: &str) -> Result<(String, String)> {
    let (key, val) = raw.split_once('=').ok_or_else(|| {
        CwError::Other(format!(
            "--env value '{}' is missing '=' (expected KEY=VAL)",
            raw
        ))
    })?;
    if key.is_empty() {
        return Err(CwError::Other(format!(
            "--env value '{}' has an empty KEY",
            raw
        )));
    }
    let bad = key.chars().next().is_some_and(|c| c.is_ascii_digit())
        || !key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_');
    if bad {
        return Err(CwError::Other(format!(
            "--env KEY '{}' must match [A-Za-z_][A-Za-z0-9_]*",
            key
        )));
    }
    Ok((key.to_string(), val.to_string()))
}

/// Validate every `--env` entry up-front and return the parsed pairs.
pub fn parse_env_entries(raw: &[String]) -> Result<Vec<(String, String)>> {
    raw.iter().map(|s| parse_env_entry(s)).collect()
}

/// Launch AI coding assistant in the specified directory.
pub fn launch_ai_tool(path: &Path, resume: bool, opts: &LaunchOptions<'_>) -> Result<()> {
    let (method, session_name) = config::resolve_term_option(opts.term_override, path)?;

    // `-T skip|none|noop` (or config method == "skip"): the user explicitly
    // asked us not to launch anything. Bail before resolving ai-tool config
    // or PATH-checking the binary so a Skip launch never errors on missing
    // tooling.
    if matches!(method, LaunchMethod::Skip) {
        return Ok(());
    }

    // Determine command. Resume always injects the tool's `--continue` /
    // `--resume` even when the user supplied `forward_args` — the user's
    // intent ("resume this") is the framing of the whole subcommand, and
    // having it silently dropped because they also passed `--model opus`
    // would be a footgun.
    let mut ai_cmd_parts = if resume {
        get_ai_tool_resume_command()?
    } else if is_claude_tool().unwrap_or(false) && session::claude_native_session_exists(path) {
        eprintln!("Found existing Claude session, using --continue");
        get_ai_tool_resume_command()?
    } else {
        get_ai_tool_command()?
    };

    if ai_cmd_parts.is_empty() {
        return Ok(());
    }

    // Forward args slot in *between* the preset's args and the (absent here)
    // prompt — same position as a hand-typed `claude --model opus`.
    ai_cmd_parts.extend(opts.forward_args.iter().cloned());

    let ai_tool_name = ai_cmd_parts[0].clone();

    if !git::has_command(&ai_tool_name) {
        println!(
            "{} {} not detected. Install it or update config with 'gw config set ai-tool <tool>'.\n",
            style("!").yellow(),
            ai_tool_name,
        );
        return Ok(());
    }

    let env = build_env_map(&ai_tool_name, opts.extra_env, opts.no_env_forward);

    // See `spawn_spec` module docstring for why the emitted line is
    // `gw _spawn-ai <path>` (no `exec` prefix) and how the raw argv flows
    // through a 0600 temp file rather than the shell line.
    maybe_inject_guard(&mut ai_cmd_parts, path)?;
    let spec = SpawnSpec::new(ai_cmd_parts, path.to_path_buf()).with_env(env);
    // The spec file is cleaned up by `spawn_spec::execute` after read; the 24h
    // `sweep_stale` at startup is the safety net for crashes between those points.
    let (cmd, _) = spawn_spec::materialize(&spec)?;

    // Dispatch to launcher. Foreground blocks on the AI process, so an RAII
    // lockfile spans the full session. Other launchers detach to a terminal
    // emulator / multiplexer and return immediately, so a lock acquired here
    // would be released before the AI session really starts — for those we
    // rely on process-cwd scanning in `busy::detect_busy` instead.
    dispatch_launch(path, method, session_name, &cmd, ai_tool_name.as_str())
}

/// Resume AI work in a worktree with context restoration.
///
/// Target resolution uses strict ordered rules: exact worktree name → exact branch
/// name → exact path. When no target is given, the current working directory is used.
pub fn resume_worktree(worktree: Option<&str>, opts: &LaunchOptions<'_>) -> Result<()> {
    let (worktree_path, branch_name) = if let Some(target) = worktree {
        let main_repo = git::get_main_repo_root(None)?;
        let strict = resolve_target_strict(&main_repo, target)?;
        let branch_name = strict.branch.unwrap_or_else(|| {
            strict
                .path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "(detached)".into())
        });
        (strict.path, branch_name)
    } else {
        // No target — use current working directory.
        let resolved = resolve_worktree_target(None, None)?;
        (resolved.path, resolved.branch)
    };

    // Change directory if specified
    if worktree.is_some() {
        let _ = std::env::set_current_dir(&worktree_path);
        println!(
            "{}\n",
            style(messages::switched_to_worktree(&worktree_path)).dim()
        );
    }

    // Check for existing session
    let has_session =
        is_claude_tool().unwrap_or(false) && session::claude_native_session_exists(&worktree_path);

    if has_session {
        println!(
            "{} Found session for branch: {}",
            style("*").green(),
            style(&branch_name).bold()
        );

        if let Some(metadata) = session::load_session_metadata(&branch_name) {
            println!("  AI tool: {}", style(&metadata.ai_tool).dim());
            println!("  Last updated: {}", style(&metadata.updated_at).dim());
        }

        if let Some(context) = session::load_context(&branch_name) {
            println!("\n{}", style("Previous context:").cyan());
            println!("{}", style(&context).dim());
        }
        println!();
    } else {
        println!(
            "{} No previous session found for branch: {}",
            style("i").yellow(),
            style(&branch_name).bold()
        );
        println!("{}\n", style("Starting fresh session...").dim());
    }

    // Save metadata and launch
    let ai_cmd = if has_session {
        get_ai_tool_resume_command()?
    } else {
        get_ai_tool_command()?
    };

    if !ai_cmd.is_empty() {
        let ai_tool_name = &ai_cmd[0];
        let _ = session::save_session_metadata(
            &branch_name,
            ai_tool_name,
            &worktree_path.to_string_lossy(),
        );

        if has_session {
            println!(
                "{} {}\n",
                style(messages::resuming_ai_tool_in(ai_tool_name)).cyan(),
                worktree_path.display()
            );
        } else {
            println!(
                "{} {}\n",
                style(messages::starting_ai_tool_in(ai_tool_name)).cyan(),
                worktree_path.display()
            );
        }

        launch_ai_tool(&worktree_path, has_session, opts)?;
    }

    Ok(())
}

/// Launch the configured AI tool inside an existing worktree.
///
/// Used by both `gw new` (after worktree creation) and `gw spawn`. Honors the
/// resolved launch method (CLI override > env > config > default).
pub fn spawn_in_worktree(
    worktree_path: &Path,
    prompt: Option<&str>,
    opts: &LaunchOptions<'_>,
) -> Result<()> {
    let (method, session_name) = config::resolve_term_option(opts.term_override, worktree_path)?;

    // `-T skip|none|noop`: caller wants the worktree set up without launching
    // anything. Skip ai-tool resolution + PATH check entirely.
    if matches!(method, LaunchMethod::Skip) {
        return Ok(());
    }

    // `--prompt` and trailing forward args are mutually exclusive: both
    // ultimately set the AI tool's prompt. Allowing both lets the user
    // accidentally end up with two prompts (the explicit one plus one
    // hidden in `forward_args`) — much better to surface this at the CLI
    // boundary than to guess an ordering.
    if prompt.is_some() && !opts.forward_args.is_empty() {
        return Err(CwError::Other(
            "--prompt / --prompt-file cannot be combined with trailing AI tool args; \
             pick one or the other"
                .to_string(),
        ));
    }

    // Build the AI tool command:
    //   <preset args...> <forward_args...> [<prompt>]
    // The prompt is appended last so the AI tool sees it as the leading
    // user message (claude/codex/gemini all accept a trailing positional).
    let mut ai_cmd_parts = get_ai_tool_command()?;
    if ai_cmd_parts.is_empty() {
        return Ok(());
    }
    ai_cmd_parts.extend(opts.forward_args.iter().cloned());
    if let Some(p) = prompt {
        ai_cmd_parts.push(p.to_string());
    }

    let ai_tool_name = ai_cmd_parts[0].clone();

    if !git::has_command(&ai_tool_name) {
        println!(
            "{} {} not detected. Install it or update config with 'gw config set ai-tool <tool>'.\n",
            style("!").yellow(),
            ai_tool_name,
        );
        return Ok(());
    }

    let env = build_env_map(&ai_tool_name, opts.extra_env, opts.no_env_forward);

    maybe_inject_guard(&mut ai_cmd_parts, worktree_path)?;
    let spec = SpawnSpec::new(ai_cmd_parts, worktree_path.to_path_buf()).with_env(env);
    let (cmd, _) = spawn_spec::materialize(&spec)?;

    dispatch_launch(
        worktree_path,
        method,
        session_name,
        &cmd,
        ai_tool_name.as_str(),
    )
}

/// Inject the gw guard PreToolUse(Bash) hook via `--settings` when the
/// configured AI tool is Claude and `ai_tool.guard` is enabled.
///
/// Inserts `--settings <inline-json>` immediately after argv\[0\], leaving
/// any subsequent positional args (delegate prompts, `--continue`, etc.)
/// in their original order.
fn maybe_inject_guard(argv: &mut Vec<String>, cwd: &Path) -> Result<()> {
    if argv.is_empty() {
        return Ok(());
    }
    if !is_claude_tool_for_cwd(cwd).unwrap_or(false) {
        return Ok(());
    }
    let cfg = load_effective_config(cwd)?;
    inject_guard_into_argv(argv, cfg.ai_tool.guard)
}

/// Pure-data version of `maybe_inject_guard`: decides injection from the
/// already-resolved `guard` flag, leaving config and tool-detection to the
/// caller. Kept separate so unit tests can exercise the argv mutation
/// without driving the config loader.
fn inject_guard_into_argv(argv: &mut Vec<String>, guard_enabled: bool) -> Result<()> {
    if !guard_enabled || argv.is_empty() {
        return Ok(());
    }
    let json = claude_settings::guard_settings_json()?;
    argv.insert(1, "--settings".to_string());
    argv.insert(2, json);
    Ok(())
}

/// Generate a session name from path with length limit.
fn generate_session_name(path: &Path) -> String {
    let config = config::load_config().unwrap_or_default();
    let prefix = &config.launch.tmux_session_prefix;
    let dir_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "worktree".to_string());

    let name = format!("{}-{}", prefix, dir_name);
    if name.len() > MAX_SESSION_NAME_LENGTH {
        name[..MAX_SESSION_NAME_LENGTH].to_string()
    } else {
        name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operations::test_env::{env_lock, EnvGuard};

    /// Resolve --settings JSON to the index immediately following argv[0].
    /// Returns (settings_json, remainder_argv_excluding_inserted_flag_pair).
    fn extract_settings(argv: &[String]) -> Option<String> {
        let pos = argv.iter().position(|s| s == "--settings")?;
        argv.get(pos + 1).cloned()
    }

    fn with_self_exe<F: FnOnce()>(f: F) {
        let _lock = env_lock();
        let _guard = EnvGuard::capture(&["CW_SPAWN_AI_BIN"]);
        std::env::set_var("CW_SPAWN_AI_BIN", "/usr/local/bin/gw");
        f();
    }

    #[test]
    fn injects_settings_after_argv0_when_enabled() {
        with_self_exe(|| {
            let mut argv = vec!["claude".to_string()];
            inject_guard_into_argv(&mut argv, true).unwrap();
            assert_eq!(argv[0], "claude");
            assert_eq!(argv[1], "--settings");
            assert_eq!(argv.len(), 3);
            let v: serde_json::Value =
                serde_json::from_str(&argv[2]).expect("settings json parses");
            assert_eq!(v["hooks"]["PreToolUse"][0]["matcher"], "Bash");
        });
    }

    #[test]
    fn noop_when_guard_disabled() {
        with_self_exe(|| {
            let mut argv = vec!["claude".to_string(), "--continue".to_string()];
            inject_guard_into_argv(&mut argv, false).unwrap();
            assert_eq!(argv, vec!["claude", "--continue"]);
        });
    }

    #[test]
    fn noop_when_argv_empty() {
        with_self_exe(|| {
            let mut argv: Vec<String> = vec![];
            inject_guard_into_argv(&mut argv, true).unwrap();
            assert!(argv.is_empty());
        });
    }

    #[test]
    fn preserves_trailing_continue_flag() {
        with_self_exe(|| {
            let mut argv = vec!["claude".to_string(), "--continue".to_string()];
            inject_guard_into_argv(&mut argv, true).unwrap();
            assert_eq!(argv[0], "claude");
            assert_eq!(argv[1], "--settings");
            assert!(extract_settings(&argv).is_some());
            assert_eq!(argv[3], "--continue");
        });
    }

    #[test]
    fn preserves_delegate_prompt_at_tail() {
        with_self_exe(|| {
            let mut argv = vec!["claude".to_string(), "do this task".to_string()];
            inject_guard_into_argv(&mut argv, true).unwrap();
            assert_eq!(argv[0], "claude");
            assert_eq!(argv[1], "--settings");
            assert!(extract_settings(&argv).is_some());
            assert_eq!(argv[3], "do this task");
        });
    }

    #[test]
    fn handles_yolo_skip_permissions_argv() {
        with_self_exe(|| {
            let mut argv = vec![
                "claude".to_string(),
                "--dangerously-skip-permissions".to_string(),
            ];
            inject_guard_into_argv(&mut argv, true).unwrap();
            // --settings goes right after argv[0], skip-permissions stays at the end
            assert_eq!(argv[0], "claude");
            assert_eq!(argv[1], "--settings");
            assert_eq!(argv[3], "--dangerously-skip-permissions");
        });
    }

    #[test]
    fn parse_env_entry_accepts_normal() {
        let (k, v) = parse_env_entry("FOO=bar").unwrap();
        assert_eq!(k, "FOO");
        assert_eq!(v, "bar");
    }

    #[test]
    fn parse_env_entry_accepts_empty_value() {
        // `--env FOO=` is the documented way to clear an auto-forwarded var.
        let (k, v) = parse_env_entry("FOO=").unwrap();
        assert_eq!(k, "FOO");
        assert_eq!(v, "");
    }

    #[test]
    fn parse_env_entry_accepts_value_with_equals() {
        // `=` is only special as a separator on the *first* occurrence —
        // everything after the first `=` is value.
        let (k, v) = parse_env_entry("FOO=a=b=c").unwrap();
        assert_eq!(k, "FOO");
        assert_eq!(v, "a=b=c");
    }

    #[test]
    fn parse_env_entry_rejects_no_equals() {
        let err = parse_env_entry("FOO").unwrap_err();
        assert!(format!("{err}").contains("missing '='"));
    }

    #[test]
    fn parse_env_entry_rejects_empty_key() {
        let err = parse_env_entry("=value").unwrap_err();
        assert!(format!("{err}").contains("empty KEY"));
    }

    #[test]
    fn parse_env_entry_rejects_digit_first_char() {
        let err = parse_env_entry("1FOO=bar").unwrap_err();
        assert!(format!("{err}").contains("[A-Za-z_]"));
    }

    #[test]
    fn parse_env_entry_rejects_dash_in_key() {
        let err = parse_env_entry("FOO-BAR=bar").unwrap_err();
        assert!(format!("{err}").contains("[A-Za-z_]"));
    }

    #[test]
    fn auto_forward_prefix_known_tools() {
        assert_eq!(auto_forward_prefix("claude"), Some("CLAUDE_"));
        assert_eq!(auto_forward_prefix("codex"), Some("CODEX_"));
        assert_eq!(auto_forward_prefix("gemini"), Some("GEMINI_"));
        assert_eq!(auto_forward_prefix("unknown-tool"), None);
    }

    #[test]
    fn build_env_map_extra_env_overrides_auto_forward() {
        // Pre-set a CLAUDE_FOO in our process so the auto-forward picks it up,
        // then verify that --env CLAUDE_FOO=override wins.
        std::env::set_var("CLAUDE_FOO_TEST_AUTO_OVR", "from-parent");
        let extra = vec![("CLAUDE_FOO_TEST_AUTO_OVR".to_string(), "override".to_string())];
        // Note: build_env_map filters by prefix "CLAUDE_", so the test var
        // must start with CLAUDE_. Both auto-forward and extra produce this
        // key — the override path inserts second so it should win.
        let env = build_env_map("claude", &extra, false);
        assert_eq!(
            env.get("CLAUDE_FOO_TEST_AUTO_OVR").map(String::as_str),
            Some("override")
        );
        std::env::remove_var("CLAUDE_FOO_TEST_AUTO_OVR");
    }

    #[test]
    fn build_env_map_no_env_forward_skips_auto() {
        std::env::set_var("CLAUDE_FOO_TEST_NO_FWD", "from-parent");
        let env = build_env_map("claude", &[], true);
        assert!(
            !env.contains_key("CLAUDE_FOO_TEST_NO_FWD"),
            "auto-forward must be suppressed by no_env_forward"
        );
        std::env::remove_var("CLAUDE_FOO_TEST_NO_FWD");
    }

    #[test]
    fn build_env_map_unknown_tool_no_auto() {
        std::env::set_var("CLAUDE_FOO_TEST_UNK", "from-parent");
        let env = build_env_map("unknown-tool", &[], false);
        assert!(env.is_empty());
        std::env::remove_var("CLAUDE_FOO_TEST_UNK");
    }
}
