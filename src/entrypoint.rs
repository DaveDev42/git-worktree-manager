//! CLI entrypoint for the `gw` binary.
//!
//! `src/bin/gw.rs` delegates to [`run`]. The library entrypoint stays
//! separate from the binary so unit tests in this module compile without
//! pulling in the binary stack-size shim.

use clap::Parser;

use crate::cli::{Cli, Commands, ConfigAction, EmitFormat};
use crate::config;
use crate::console as cwconsole;
use crate::constants;
use crate::cwshare_setup;
use crate::error::{CwError, Result};
use crate::operations::ai_tools::LaunchOptions;
use crate::operations::{
    ai_tools, claude_worktree, config_ops, diagnostics, display, exec, guard, helpers, path_cmd,
    run, setup_claude, spawn_spec, worktree,
};
use crate::resolve_prompt;
use crate::shell_functions;
use crate::tui;
use crate::update;
use std::io::{IsTerminal, Read};

/// Error returned when `--prompt`/`--prompt-file` and trailing AI-tool args
/// are both provided. Both ultimately set the AI tool's prompt, so mixing them
/// would silently produce two conflicting prompts.
const ERR_PROMPT_AND_FORWARD: &str = "--prompt / --prompt-file cannot be combined with trailing \
     AI tool args; pick one or the other";

pub fn run() {
    tui::install_panic_hook();
    let cli = Cli::parse();

    if let Some(ref shell_name) = cli.generate_completion {
        generate_completions(shell_name);
        return;
    }

    // Skip startup checks for internal commands (shell-completion helpers,
    // cache refresh) — they are invoked by the shell on every keystroke, so
    // paying for update-check / prompts would compound latency and risk
    // recursive re-entry into the update flow.
    let is_internal = matches!(
        &cli.command,
        Some(
            Commands::UpdateCache
                | Commands::CompleteTargets
                | Commands::Path { .. }
                | Commands::ShellFunction { .. }
                | Commands::SpawnAi { .. }
                | Commands::Guard { .. }
                | Commands::ClaudeWorktreeCreate
                | Commands::ClaudeWorktreeRemove
        )
    );

    if !is_internal {
        crate::operations::spawn_spec::sweep_stale();
        update::check_for_update_if_needed();
    }

    // `gw config` is the user's tool for inspecting/editing the same file the
    // one-time shell-completion hint persists to. Triggering the prompt here
    // would seed a fresh global config file as a side-effect of `gw config
    // list` (or even `gw config get`), surprising the user — and worse, makes
    // every `[default]` row of `list` render as `[global]` once the autosave
    // lands. Skip the hint for the `config` family; users who want it can
    // discover it via any other command (or `gw shell-setup` directly).
    let skip_shell_completion_prompt =
        is_internal || matches!(&cli.command, Some(Commands::Config { .. }));

    if !skip_shell_completion_prompt {
        config::prompt_shell_completion_setup();
    }

    let result = match cli.command {
        Some(Commands::List) => display::list_worktrees(),
        Some(Commands::Ls) => display::list_worktrees_tsv(),
        Some(Commands::New {
            name,
            path,
            base,
            term,
            prompt,
            prompt_file,
            no_env_forward,
            emit,
            forward_args,
        }) => (|| -> Result<()> {
            // Reject --prompt + trailing forward args at dispatch time —
            // both ultimately set the AI tool's prompt, so allowing both
            // would silently produce two prompts. Surface this before
            // anything touches disk.
            if (prompt.is_some() || prompt_file.is_some()) && !forward_args.is_empty() {
                return Err(CwError::Other(ERR_PROMPT_AND_FORWARD.to_string()));
            }
            // Catch `gw new <name> -- --prompt-file <path>` — clap would
            // forward those gw-owned flags verbatim into the AI tool's
            // argv, and `claude` would then die with "unknown option".
            reject_gw_flags_in_forward(&forward_args)?;
            // Resolve the prompt first so a missing file or unreadable stdin
            // fails before any interactive side effects (worktree creation,
            // AI-tool launch) leave the tree in a half-configured state.
            let resolved = resolve_prompt(
                prompt,
                prompt_file.as_deref(),
                || std::io::stdin().is_terminal(),
                || {
                    let mut buf = String::new();
                    std::io::stdin().read_to_string(&mut buf)?;
                    Ok(buf)
                },
            )?;
            // Pre-flight `-T <method>` so a typo (`-T does-not-exist`) errors
            // before we create a worktree on disk. The launch path inside
            // create_worktree swallows spawn errors with `let _ = …`, which
            // would otherwise leave a phantom worktree on a bad alias.
            // Skip / none / noop is a valid value here, so this still parses.
            let _ = config::parse_term_option(term.as_deref())?;
            cwshare_setup::prompt_cwshare_setup();

            // --emit json implies -T skip: the caller reads worktree_path from
            // stdout, so spawning a terminal would race with that contract.
            let effective_term = if emit == EmitFormat::Json && term.is_none() {
                Some("skip".to_string())
            } else {
                term
            };
            let opts = LaunchOptions {
                term_override: effective_term.as_deref(),
                forward_args: &forward_args,
                no_env_forward,
            };
            worktree::create_worktree(
                &name,
                base.as_deref(),
                path.as_deref(),
                resolved.as_deref(),
                &opts,
                emit,
            )?;
            Ok(())
        })(),

        Some(Commands::Resume {
            branch,
            term,
            no_env_forward,
            forward_args,
        }) => (|| -> Result<()> {
            // clap's trailing_var_arg + Option<positional> combo lets `--`
            // get absorbed by the optional positional: `gw resume -- --model
            // opus` parses as branch=Some("--model"), forward_args=["opus"].
            // Detect a hyphen-led "branch" and lift it into forward_args.
            let (branch, forward_args) = lift_dash_target(branch, forward_args);
            // `gw resume` has no `--prompt` of its own, but we still want
            // `gw resume <branch> -- --prompt-file <path>` to fail fast
            // here rather than inside the spawned terminal where the user
            // can't see the AI tool's stderr.
            reject_gw_flags_in_forward(&forward_args)?;
            let opts = LaunchOptions {
                term_override: term.as_deref(),
                forward_args: &forward_args,
                no_env_forward,
            };
            ai_tools::resume_worktree(branch.as_deref(), &opts)
        })(),

        Some(Commands::Spawn {
            target,
            term,
            prompt,
            prompt_file,
            no_env_forward,
            forward_args,
        }) => (|| -> Result<()> {
            // See Resume arm: clap absorbs `--` into the optional positional.
            let (target, forward_args) = lift_dash_target(target, forward_args);
            // Same prompt/forward conflict as `gw new`.
            if (prompt.is_some() || prompt_file.is_some()) && !forward_args.is_empty() {
                return Err(CwError::Other(ERR_PROMPT_AND_FORWARD.to_string()));
            }
            // Same `-- --prompt-file` leak as `gw new`.
            reject_gw_flags_in_forward(&forward_args)?;
            let resolved_prompt = resolve_prompt(
                prompt,
                prompt_file.as_deref(),
                || std::io::stdin().is_terminal(),
                || {
                    let mut buf = String::new();
                    std::io::stdin().read_to_string(&mut buf)?;
                    Ok(buf)
                },
            )?;
            let cwd = std::env::current_dir()?;
            let target_path = match target {
                Some(t) => {
                    let main_repo = crate::git::get_main_repo_root(Some(&cwd))?;
                    helpers::resolve_target_strict(&main_repo, &t)?.path
                }
                None => crate::git::get_repo_root(Some(&cwd))?,
            };
            let opts = LaunchOptions {
                term_override: term.as_deref(),
                forward_args: &forward_args,
                no_env_forward,
            };
            ai_tools::spawn_in_worktree(&target_path, resolved_prompt.as_deref(), &opts)
        })(),

        Some(Commands::Rm {
            targets,
            interactive,
            dry_run,
            keep_branch,
            delete_remote,
            force,
            no_force,
        }) => {
            let flags = crate::operations::worktree::RmFlags {
                keep_branch,
                delete_remote,
                git_force: !no_force,
                allow_busy: force,
            };
            match crate::operations::rm_batch::rm_worktrees(targets, interactive, dry_run, flags) {
                Ok(0) => Ok(()),
                Ok(code) => Err(crate::error::CwError::ExitCode(code)),
                Err(e) => Err(e),
            }
        }

        Some(Commands::Doctor {
            session_start,
            quiet,
        }) => diagnostics::doctor(session_start, quiet),
        Some(Commands::Run {
            only,
            no_main,
            jobs,
            continue_on_error,
            cmd,
        }) => (|| -> Result<()> {
            let cwd = std::env::current_dir()?;
            let code = run::run_in_scope(
                &cwd,
                &cmd,
                only.as_deref(),
                no_main,
                jobs,
                continue_on_error,
            )?;
            if code != 0 {
                return Err(crate::error::CwError::ExitCode(code));
            }
            Ok(())
        })(),

        Some(Commands::Exec { target, cmd }) => (|| -> Result<()> {
            let cwd = std::env::current_dir()?;
            let mut out = std::io::stdout().lock();
            let code = exec::exec_in_target(&cwd, &target, &cmd, &mut out)?;
            if code != 0 {
                return Err(crate::error::CwError::ExitCode(code));
            }
            Ok(())
        })(),

        Some(Commands::Guard { tool_input }) => guard::run(&tool_input),

        Some(Commands::ClaudeWorktreeCreate) => claude_worktree::run_create(),
        Some(Commands::ClaudeWorktreeRemove) => claude_worktree::run_remove(),

        Some(Commands::SetupClaude) => setup_claude::setup_claude(),

        Some(Commands::Config { action }) => match action {
            ConfigAction::List => config_ops::list_cmd(),
            ConfigAction::Get { key } => config_ops::get_cmd(key),
            ConfigAction::Set { key, value, repo } => {
                let scope = if repo {
                    config_ops::Scope::Repo
                } else {
                    config_ops::Scope::Global
                };
                config_ops::set_cmd(key, &value, scope)
            }
            ConfigAction::Edit => crate::tui::config_editor::run(),
        },

        Some(Commands::Upgrade { yes }) => {
            update::upgrade(yes);
            Ok(())
        }

        Some(Commands::ShellSetup) => {
            shell_setup();
            Ok(())
        }

        Some(Commands::Path {
            branch,
            list_branches,
            interactive,
        }) => path_cmd::worktree_path(branch.as_deref(), list_branches, interactive),

        Some(Commands::ShellFunction { shell }) => match shell_functions::generate(&shell) {
            Some(output) => {
                print!("{}", output);
                Ok(())
            }
            None => Err(CwError::Config(format!(
                "Unsupported shell: {}. Use bash, zsh, fish, or powershell.",
                shell
            ))),
        },

        Some(Commands::UpdateCache) => {
            update::refresh_cache();
            Ok(())
        }

        Some(Commands::CompleteTargets) => crate::operations::complete::print_completion_targets(),

        Some(Commands::SpawnAi { spec }) => {
            // Pre-spawn failures (read/parse/chdir) exit 127 — the shell
            // "command not found / could not start" convention. Post-spawn
            // failures exit from inside `execute` directly, also with 127.
            // Inner errors already carry the "spawn-ai:" prefix via their
            // CwError::Other messages, so we print them verbatim.
            let resolved = match spec {
                Some(p) => p,
                None => match spawn_spec::resolve_last_for_cwd() {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("{}", e);
                        std::process::exit(127);
                    }
                },
            };
            if let Err(e) = spawn_spec::execute(&resolved) {
                eprintln!("{}", e);
                std::process::exit(127);
            }
            Ok(())
        }

        None => Ok(()),
    };

    if let Err(e) = result {
        // ExitCode carries a specific exit status from callers that have
        // already produced their own user-facing output (e.g. the multi-target
        // delete orchestrator). Exit silently with that code instead of the
        // generic "Error: …" print.
        if let CwError::ExitCode(code) = e {
            std::process::exit(code);
        }
        cwconsole::print_error(&format!("Error: {}", e));
        std::process::exit(1);
    }
}

fn generate_completions(shell_name: &str) {
    use clap::CommandFactory;
    use clap_complete::{generate, Shell};

    let shell = match shell_name.to_lowercase().as_str() {
        "bash" => Shell::Bash,
        "zsh" => Shell::Zsh,
        "fish" => Shell::Fish,
        "powershell" | "pwsh" => Shell::PowerShell,
        "elvish" => Shell::Elvish,
        _ => {
            eprintln!(
                "Unsupported shell: {}. Use bash, zsh, fish, powershell, or elvish.",
                shell_name
            );
            std::process::exit(1);
        }
    };

    let mut cmd = Cli::command();
    generate(shell, &mut cmd, "gw", &mut std::io::stdout());
}

fn shell_setup() {
    let shell_env = std::env::var("SHELL").unwrap_or_default();
    let is_powershell = cfg!(target_os = "windows") || std::env::var("PSModulePath").is_ok();

    let home = constants::home_dir_or_fallback();
    let (shell_name, profile_path) = if shell_env.contains("zsh") {
        ("zsh", Some(home.join(".zshrc")))
    } else if shell_env.contains("bash") {
        ("bash", Some(home.join(".bashrc")))
    } else if shell_env.contains("fish") {
        (
            "fish",
            Some(home.join(".config").join("fish").join("config.fish")),
        )
    } else if is_powershell {
        ("powershell", None::<std::path::PathBuf>)
    } else {
        println!("Could not detect your shell automatically.\n");
        println!("Please manually add the gw-cd function to your shell:\n");
        println!("  bash/zsh:    source <(gw _shell-function bash)");
        println!("  fish:        gw _shell-function fish | source");
        println!("  PowerShell:  gw _shell-function powershell | Out-String | Invoke-Expression");
        return;
    };

    println!("Detected shell: {}\n", shell_name);

    if shell_name == "powershell" {
        println!("To enable gw-cd in PowerShell, add the following to your $PROFILE:\n");
        println!("  gw _shell-function powershell | Out-String | Invoke-Expression\n");
        println!("To find your PowerShell profile location, run: $PROFILE");
        println!(
            "\nIf the profile file doesn't exist, create it with: New-Item -Path $PROFILE -ItemType File -Force"
        );
        return;
    }

    let shell_function_line = match shell_name {
        "fish" => "gw _shell-function fish | source".to_string(),
        _ => format!("source <(gw _shell-function {})", shell_name),
    };

    if let Some(ref path) = profile_path {
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(path) {
                if content.contains("gw _shell-function") || content.contains("gw-cd") {
                    println!(
                        "{}",
                        console::style("Shell integration is already installed.").green()
                    );
                    println!("  Found in: {}\n", path.display());

                    refresh_shell_cache(shell_name);

                    println!("\nRestart your shell or run: source {}", path.display());
                    return;
                }
            }
        }
    }

    println!("Setup shell integration?\n");
    println!(
        "This will add the following to {}:",
        profile_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "your profile".to_string())
    );

    println!(
        "\n  # git-worktree-manager shell integration{}",
        if matches!(shell_name, "zsh" | "bash") {
            " (gw-cd + tab completion)"
        } else {
            ""
        }
    );
    println!("  {}\n", shell_function_line);

    print!("Add to your shell profile? [Y/n]: ");
    use std::io::Write;
    let _ = std::io::stdout().flush();

    let mut input = String::new();
    let _ = std::io::stdin().read_line(&mut input);
    let input = input.trim().to_lowercase();

    if !input.is_empty() && input != "y" && input != "yes" {
        println!("\nSetup cancelled.");
        return;
    }

    let Some(ref path) = profile_path else {
        return;
    };

    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let comment_suffix = if matches!(shell_name, "zsh" | "bash") {
        " (gw-cd + tab completion)"
    } else {
        ""
    };
    let append = format!(
        "\n# git-worktree-manager shell integration{}\n{}\n",
        comment_suffix, shell_function_line
    );

    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        Ok(mut f) => {
            let _ = f.write_all(append.as_bytes());

            if let Ok(mut cfg) = config::load_config() {
                cfg.shell_completion.installed = true;
                cfg.shell_completion.prompted = true;
                let _ = config::save_config(&cfg);
            }

            println!("\n* Successfully added to {}", path.display());

            refresh_shell_cache(shell_name);

            println!("\nNext steps:");
            println!("  1. Restart your shell or run: source {}", path.display());
            println!("  2. Try directory navigation: gw-cd <branch-name>");
            println!("  3. Try tab completion: gw <TAB> or gw new <TAB>");
        }
        Err(e) => {
            println!("\nError: Failed to update {}: {}", path.display(), e);
            println!("\nTo install manually, add the lines shown above to your profile");
        }
    }
}

/// Refresh cached shell function files to pick up new features.
fn refresh_shell_cache(shell_name: &str) {
    let home = constants::home_dir_or_fallback();

    let cache_paths = [
        home.join(".cache").join("gw-shell-function.zsh"),
        home.join(".cache").join("gw-shell-function.bash"),
        home.join(".cache").join("gw-shell-function.fish"),
    ];

    let mut refreshed = false;
    for cache_path in &cache_paths {
        if !cache_path.exists() {
            continue;
        }
        let cache_shell = cache_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        if let Some(content) = shell_functions::generate(cache_shell) {
            if std::fs::write(cache_path, content).is_ok() {
                println!(
                    "  {} {}",
                    console::style("Refreshed cache:").dim(),
                    cache_path.display()
                );
                refreshed = true;
            }
        }
    }

    if refreshed {
        return;
    }

    let cache_path = home
        .join(".cache")
        .join(format!("gw-shell-function.{}", shell_name));
    if let Some(content) = shell_functions::generate(shell_name) {
        if let Some(cache_dir) = cache_path.parent() {
            let _ = std::fs::create_dir_all(cache_dir);
        }
        if std::fs::write(&cache_path, &content).is_ok() {
            println!(
                "  {} {}",
                console::style("Created cache:").dim(),
                cache_path.display()
            );
        }
    }
}

/// Reject gw's own `--prompt` / `--prompt-file` flags when they leak into
/// the trailing forward-args slot (typically because the caller wrote
/// `gw new <name> -- --prompt-file <path>`, which clap dutifully forwards
/// to the AI tool — `claude` then rejects the unknown option). We surface
/// the error at dispatch time instead of letting it fail inside the spawned
/// terminal where the user can't see it.
///
/// We scan every element rather than stopping at the first non-flag: these
/// tokens are gw-owned and have no meaning to any AI tool we support, so
/// catching them no matter where they sit in the forward list is strictly
/// safer than letting the spawned process fail. If a real AI tool ever
/// gains an identically-named flag, this is a one-line carve-out.
fn reject_gw_flags_in_forward(forward_args: &[String]) -> Result<()> {
    const GW_PROMPT_FLAGS: &[&str] = &["--prompt", "--prompt-file"];
    for arg in forward_args {
        if !arg.starts_with("--") {
            continue;
        }
        let head = arg.split_once('=').map(|(h, _)| h).unwrap_or(arg.as_str());
        if GW_PROMPT_FLAGS.contains(&head) {
            return Err(CwError::Other(format!(
                "{head} is a gw option, not an AI tool option — drop the `--` \
                 separator so gw consumes the flag itself (write `{head} \
                 <value>` without `--` in front of it)"
            )));
        }
    }
    Ok(())
}

/// `gw spawn -- --model opus` and `gw resume -- --model opus` parse, under
/// clap's `trailing_var_arg=true` + `Option<String>` positional combo, as
/// `target=Some("--model")`, `forward_args=["opus"]` — clap silently absorbs
/// the `--` into the optional positional. A real worktree name can never
/// start with `-` (git rejects it), so a hyphen-led "target" is unambiguously
/// a misparsed forward arg. Lift it (and any captured forward args) back into
/// forward_args, with `target` cleared so the dispatcher falls through to
/// "current worktree".
fn lift_dash_target(
    target: Option<String>,
    forward_args: Vec<String>,
) -> (Option<String>, Vec<String>) {
    match target {
        Some(t) if t.starts_with('-') => {
            let mut lifted = Vec::with_capacity(forward_args.len() + 1);
            lifted.push(t);
            lifted.extend(forward_args);
            (None, lifted)
        }
        other => (other, forward_args),
    }
}

#[cfg(test)]
mod tests {
    use super::{lift_dash_target, reject_gw_flags_in_forward};

    fn forward(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn reject_forward_args_passes_when_empty() {
        reject_gw_flags_in_forward(&[]).unwrap();
    }

    #[test]
    fn reject_forward_args_passes_ai_tool_flags() {
        // `--model opus`, `--resume`, `--continue`, `--print` — all legitimate
        // claude/codex flags; must flow through untouched.
        reject_gw_flags_in_forward(&forward(&["--model", "opus", "--resume"])).unwrap();
        reject_gw_flags_in_forward(&forward(&["--print"])).unwrap();
    }

    #[test]
    fn reject_forward_args_rejects_prompt_file_leading() {
        let err = reject_gw_flags_in_forward(&forward(&["--prompt-file", "/tmp/p.txt"]))
            .expect_err("must reject");
        let msg = format!("{err}");
        assert!(msg.contains("--prompt-file"), "unexpected msg: {msg}");
        assert!(msg.contains("gw option"), "unexpected msg: {msg}");
    }

    #[test]
    fn reject_forward_args_rejects_prompt_leading() {
        let err =
            reject_gw_flags_in_forward(&forward(&["--prompt", "hi"])).expect_err("must reject");
        assert!(format!("{err}").contains("--prompt"));
    }

    #[test]
    fn reject_forward_args_rejects_equals_form() {
        let err = reject_gw_flags_in_forward(&forward(&["--prompt-file=/tmp/p.txt"]))
            .expect_err("must reject");
        assert!(format!("{err}").contains("--prompt-file"));
        let err =
            reject_gw_flags_in_forward(&forward(&["--prompt=hello"])).expect_err("must reject");
        assert!(format!("{err}").contains("--prompt"));
    }

    #[test]
    fn reject_forward_args_rejects_prompt_after_positional() {
        // Catch even when the leaked gw flag follows a positional or another
        // flag's value — claude/codex/gemini have no `--prompt-file` of their
        // own, so any occurrence is a misroute regardless of position.
        let err = reject_gw_flags_in_forward(&forward(&["some-prompt", "--prompt-file", "/tmp/p"]))
            .expect_err("must reject");
        assert!(format!("{err}").contains("--prompt-file"));
    }

    #[test]
    fn reject_forward_args_rejects_prompt_after_other_flags() {
        // Mixed: a legitimate AI-tool flag and its value, followed by a
        // leaked gw flag.
        let err =
            reject_gw_flags_in_forward(&forward(&["--model", "opus", "--prompt-file", "/tmp/p"]))
                .expect_err("must reject");
        assert!(format!("{err}").contains("--prompt-file"));
    }

    #[test]
    fn reject_forward_args_ignores_short_dash_and_bare_dash() {
        // `-` (read stdin convention) and `-x`-style short flags must not
        // trip the guard — we only match the two specific long flags.
        reject_gw_flags_in_forward(&forward(&["-"])).unwrap();
        reject_gw_flags_in_forward(&forward(&["-p", "hello"])).unwrap();
    }

    #[test]
    fn lift_dash_target_lifts_hyphen_target() {
        let (target, fwd) = lift_dash_target(
            Some("--model".to_string()),
            vec!["opus".to_string(), "--resume".to_string()],
        );
        assert_eq!(target, None);
        assert_eq!(fwd, vec!["--model", "opus", "--resume"]);
    }

    #[test]
    fn lift_dash_target_passes_through_normal_target() {
        let (target, fwd) = lift_dash_target(
            Some("feat-x".to_string()),
            vec!["--model".to_string(), "opus".to_string()],
        );
        assert_eq!(target.as_deref(), Some("feat-x"));
        assert_eq!(fwd, vec!["--model", "opus"]);
    }

    #[test]
    fn lift_dash_target_handles_none_target() {
        let (target, fwd) = lift_dash_target(None, vec![]);
        assert_eq!(target, None);
        assert!(fwd.is_empty());
    }

    #[test]
    fn lift_dash_target_lifts_with_no_forward_args() {
        // `gw spawn -- --model` (no value) — pathological but still well-defined.
        let (target, fwd) = lift_dash_target(Some("--model".to_string()), vec![]);
        assert_eq!(target, None);
        assert_eq!(fwd, vec!["--model"]);
    }
}
