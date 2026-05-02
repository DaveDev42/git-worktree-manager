/// AI tool integration operations.
///
/// Handles launching AI coding assistants in various terminal environments.
use std::path::Path;

use console::style;

use crate::config::{self, get_ai_tool_command, get_ai_tool_resume_command, is_claude_tool};
use crate::constants::{LaunchMethod, MAX_SESSION_NAME_LENGTH};
use crate::error::Result;
use crate::git;
use crate::messages;
use crate::session;

use super::helpers::{resolve_target_strict, resolve_worktree_target};
use super::launchers;
use super::spawn_spec::{self, SpawnSpec};

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

/// Launch AI coding assistant in the specified directory.
pub fn launch_ai_tool(path: &Path, resume: bool) -> Result<()> {
    let method = config::get_default_launch_method()?;

    // Determine command
    let ai_cmd_parts = if resume {
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

    let ai_tool_name = ai_cmd_parts[0].clone();

    if !git::has_command(&ai_tool_name) {
        println!(
            "{} {} not detected. Install it or update config with 'cw config set ai-tool <tool>'.\n",
            style("!").yellow(),
            ai_tool_name,
        );
        return Ok(());
    }

    // See `spawn_spec` module docstring for why the emitted line is
    // `gw _spawn-ai <path>` (no `exec` prefix) and how the raw argv flows
    // through a 0600 temp file rather than the shell line.
    let spec = SpawnSpec::new(ai_cmd_parts, path.to_path_buf());
    // The spec file is cleaned up by `spawn_spec::execute` after read; the 24h
    // `sweep_stale` at startup is the safety net for crashes between those points.
    let (cmd, _) = spawn_spec::materialize(&spec)?;

    // Dispatch to launcher. Foreground blocks on the AI process, so an RAII
    // lockfile spans the full session. Other launchers detach to a terminal
    // emulator / multiplexer and return immediately, so a lock acquired here
    // would be released before the AI session really starts — for those we
    // rely on process-cwd scanning in `busy::detect_busy` instead.
    dispatch_launch(path, method, None, &cmd, ai_tool_name.as_str())
}

/// Resume AI work in a worktree with context restoration.
///
/// Target resolution uses strict ordered rules: exact worktree name → exact branch
/// name → exact path. When no target is given, the current working directory is used.
pub fn resume_worktree(worktree: Option<&str>) -> Result<()> {
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

        launch_ai_tool(&worktree_path, has_session)?;
    }

    Ok(())
}

/// Launch the configured AI tool inside an existing worktree.
///
/// Used by both `gw new` (after worktree creation) and `gw spawn`. Honors the
/// default launch method from config; no terminal-override / fg / bg knobs.
pub fn spawn_in_worktree(worktree_path: &Path, prompt: Option<&str>) -> Result<()> {
    let method = config::get_default_launch_method()?;

    let ai_cmd_parts = if let Some(p) = prompt {
        config::get_ai_tool_merge_command(p)?
    } else {
        get_ai_tool_command()?
    };

    if ai_cmd_parts.is_empty() {
        return Ok(());
    }

    let ai_tool_name = ai_cmd_parts[0].clone();

    if !git::has_command(&ai_tool_name) {
        println!(
            "{} {} not detected. Install it or update config with 'cw config set ai-tool <tool>'.\n",
            style("!").yellow(),
            ai_tool_name,
        );
        return Ok(());
    }

    let spec = SpawnSpec::new(ai_cmd_parts, worktree_path.to_path_buf());
    let (cmd, _) = spawn_spec::materialize(&spec)?;

    dispatch_launch(worktree_path, method, None, &cmd, ai_tool_name.as_str())
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
