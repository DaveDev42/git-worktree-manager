/// AI tool integration operations.
///
/// Mirrors src/claude_worktree/operations/ai_tools.py (752 lines).
/// Handles launching AI coding assistants in various terminal environments.
use std::collections::HashMap;
use std::path::Path;

use console::style;

use crate::config::{
    self, get_ai_tool_command, get_ai_tool_resume_command, is_claude_tool, parse_term_option,
};
use crate::constants::{
    format_config_key, LaunchMethod, CONFIG_KEY_BASE_BRANCH, MAX_SESSION_NAME_LENGTH,
};
use crate::error::Result;
use crate::git;
use crate::hooks;
use crate::session;

use super::helpers::resolve_worktree_target;
use super::launchers;

/// Launch AI coding assistant in the specified directory.
pub fn launch_ai_tool(
    path: &Path,
    term: Option<&str>,
    resume: bool,
    prompt: Option<&str>,
) -> Result<()> {
    let (method, session_name) = parse_term_option(term)?;

    // Determine command
    let ai_cmd_parts = if let Some(p) = prompt {
        config::get_ai_tool_merge_command(p)?
    } else if resume {
        get_ai_tool_resume_command()?
    } else {
        // Smart --continue for Claude
        if is_claude_tool().unwrap_or(false) && session::claude_native_session_exists(path) {
            eprintln!("Found existing Claude session, using --continue");
            get_ai_tool_resume_command()?
        } else {
            get_ai_tool_command()?
        }
    };

    if ai_cmd_parts.is_empty() {
        return Ok(());
    }

    let ai_tool_name = &ai_cmd_parts[0];

    if !git::has_command(ai_tool_name) {
        println!(
            "{} {} not detected. Install it or update config with 'cw config set ai-tool <tool>'.\n",
            style("!").yellow(),
            ai_tool_name,
        );
        return Ok(());
    }

    // Build shell command string
    let cmd = shell_quote_join(&ai_cmd_parts);

    // Dispatch to launcher
    match method {
        LaunchMethod::Foreground => {
            println!(
                "{}\n",
                style(format!("Starting {} (Ctrl+C to exit)...", ai_tool_name)).cyan()
            );
            launchers::foreground::run(path, &cmd);
        }
        LaunchMethod::Detach => {
            launchers::detached::run(path, &cmd);
            println!(
                "{} {} detached (survives terminal close)\n",
                style("*").green().bold(),
                ai_tool_name
            );
        }
        // iTerm
        LaunchMethod::ItermWindow => launchers::iterm::launch_window(path, &cmd, ai_tool_name)?,
        LaunchMethod::ItermTab => launchers::iterm::launch_tab(path, &cmd, ai_tool_name)?,
        LaunchMethod::ItermPaneH => launchers::iterm::launch_pane(path, &cmd, ai_tool_name, true)?,
        LaunchMethod::ItermPaneV => launchers::iterm::launch_pane(path, &cmd, ai_tool_name, false)?,
        // tmux
        LaunchMethod::Tmux => {
            let sn = session_name.unwrap_or_else(|| generate_session_name(path));
            launchers::tmux::launch_session(path, &cmd, ai_tool_name, &sn)?;
        }
        LaunchMethod::TmuxWindow => launchers::tmux::launch_window(path, &cmd, ai_tool_name)?,
        LaunchMethod::TmuxPaneH => launchers::tmux::launch_pane(path, &cmd, ai_tool_name, true)?,
        LaunchMethod::TmuxPaneV => launchers::tmux::launch_pane(path, &cmd, ai_tool_name, false)?,
        // Zellij
        LaunchMethod::Zellij => {
            let sn = session_name.unwrap_or_else(|| generate_session_name(path));
            launchers::zellij::launch_session(path, &cmd, ai_tool_name, &sn)?;
        }
        LaunchMethod::ZellijTab => launchers::zellij::launch_tab(path, &cmd, ai_tool_name)?,
        LaunchMethod::ZellijPaneH => {
            launchers::zellij::launch_pane(path, &cmd, ai_tool_name, true)?
        }
        LaunchMethod::ZellijPaneV => {
            launchers::zellij::launch_pane(path, &cmd, ai_tool_name, false)?
        }
        // WezTerm
        LaunchMethod::WeztermWindow => launchers::wezterm::launch_window(path, &cmd, ai_tool_name)?,
        LaunchMethod::WeztermTab => launchers::wezterm::launch_tab(path, &cmd, ai_tool_name)?,
        LaunchMethod::WeztermPaneH => {
            launchers::wezterm::launch_pane(path, &cmd, ai_tool_name, true)?
        }
        LaunchMethod::WeztermPaneV => {
            launchers::wezterm::launch_pane(path, &cmd, ai_tool_name, false)?
        }
    }

    Ok(())
}

/// Resume AI work in a worktree with context restoration.
pub fn resume_worktree(worktree: Option<&str>, term: Option<&str>) -> Result<()> {
    let (worktree_path, branch_name, worktree_repo) = resolve_worktree_target(worktree, None)?;

    // Pre-resume hooks
    let base_key = format_config_key(CONFIG_KEY_BASE_BRANCH, &branch_name);
    let base_branch = git::get_config(&base_key, Some(&worktree_repo)).unwrap_or_default();

    let mut hook_ctx = HashMap::new();
    hook_ctx.insert("branch".into(), branch_name.clone());
    hook_ctx.insert("base_branch".into(), base_branch);
    hook_ctx.insert(
        "worktree_path".into(),
        worktree_path.to_string_lossy().to_string(),
    );
    hook_ctx.insert(
        "repo_path".into(),
        worktree_repo.to_string_lossy().to_string(),
    );
    hook_ctx.insert("event".into(), "resume.pre".into());
    hook_ctx.insert("operation".into(), "resume".into());
    hooks::run_hooks(
        "resume.pre",
        &hook_ctx,
        Some(&worktree_path),
        Some(&worktree_repo),
    )?;

    // Change directory if specified
    if worktree.is_some() {
        let _ = std::env::set_current_dir(&worktree_path);
        println!(
            "{}\n",
            style(format!("Switched to worktree: {}", worktree_path.display())).dim()
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
                style(format!("Resuming {} in:", ai_tool_name)).cyan(),
                worktree_path.display()
            );
        } else {
            println!(
                "{} {}\n",
                style(format!("Starting {} in:", ai_tool_name)).cyan(),
                worktree_path.display()
            );
        }

        launch_ai_tool(&worktree_path, term, has_session, None)?;
    }

    // Post-resume hooks
    hook_ctx.insert("event".into(), "resume.post".into());
    let _ = hooks::run_hooks(
        "resume.post",
        &hook_ctx,
        Some(&worktree_path),
        Some(&worktree_repo),
    );

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

/// Shell-quote and join command parts.
fn shell_quote_join(parts: &[String]) -> String {
    parts
        .iter()
        .map(|p| {
            if p.contains(char::is_whitespace) || p.contains('\'') || p.contains('"') {
                format!("'{}'", p.replace('\'', "'\\''"))
            } else {
                p.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
