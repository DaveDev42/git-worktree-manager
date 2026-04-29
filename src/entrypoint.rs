//! CLI entrypoint — shared between the `gw` and `cw` binaries.
//!
//! Both binary targets (`src/bin/gw.rs`, `src/bin/cw.rs`) delegate to
//! [`run`]. Keeping the logic here avoids compiling the same source file
//! twice, which triggered Cargo's "file present in multiple build targets"
//! warning under the previous layout.

use clap::Parser;

use crate::cli::{BackupAction, Cli, Commands, ConfigAction, HookAction, StashAction};
use crate::config;
use crate::console as cwconsole;
use crate::constants;
use crate::cwshare_setup;
use crate::error::{CwError, Result};
use crate::hooks;
use crate::operations::{
    ai_tools, backup, clean, config_ops, diagnostics, display, git_ops, global_ops, guard, helpers,
    path_cmd, setup_claude, shell, spawn_spec, stash, worktree,
};
use crate::resolve_prompt;
use crate::shell_functions;
use crate::tui;
use crate::update;
use std::io::Read;

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
                | Commands::ConfigKeys
                | Commands::TermValues
                | Commands::PresetNames
                | Commands::HookEvents
                | Commands::Path { .. }
                | Commands::ShellFunction { .. }
                | Commands::SpawnAi { .. }
                | Commands::Guard { .. }
        )
    );

    if !is_internal {
        crate::operations::spawn_spec::sweep_stale();
        update::check_for_update_if_needed();
    }

    if !is_internal {
        config::prompt_shell_completion_setup();
    }

    helpers::set_global_mode(cli.global);

    let result = match cli.command {
        Some(Commands::List { cache }) => {
            let no_cache = cache.no_cache;
            if cli.global {
                global_ops::global_list_worktrees(no_cache)
            } else {
                display::list_worktrees(no_cache)
            }
        }
        Some(Commands::Status { cache }) => display::show_status(cache.no_cache),
        Some(Commands::Tree { cache }) => display::show_tree(cache.no_cache),
        Some(Commands::Stats { cache }) => display::show_stats(cache.no_cache),
        Some(Commands::Diff {
            branch1,
            branch2,
            summary,
            files,
        }) => display::diff_worktrees(&branch1, &branch2, summary, files),

        Some(Commands::Config { action }) => match action {
            ConfigAction::Show => config::show_config().map(|output| println!("{}", output)),
            ConfigAction::List => config::list_config(),
            ConfigAction::Get { key } => config::get_config_value(&key),
            ConfigAction::Set { key, value } => config::set_config_value(&key, &value),
            ConfigAction::UsePreset { name } => config::use_preset(&name),
            ConfigAction::ListPresets => {
                println!("{}", config::list_presets());
                Ok(())
            }
            ConfigAction::Reset => config::reset_config(),
        },

        Some(Commands::New {
            name,
            path,
            base,
            no_term,
            term,
            bg,
            fg,
            prompt,
            prompt_file,
            prompt_stdin,
        }) => (|| -> Result<()> {
            // Resolve the prompt first so a missing file or unreadable stdin
            // fails before any interactive side effects (worktree creation,
            // AI-tool launch) leave the tree in a half-configured state.
            let resolved = resolve_prompt(prompt, prompt_file.as_deref(), prompt_stdin, || {
                let mut buf = String::new();
                std::io::stdin().read_to_string(&mut buf)?;
                Ok(buf)
            })?;

            cwshare_setup::prompt_cwshare_setup();

            worktree::create_worktree(
                &name,
                base.as_deref(),
                path.as_deref(),
                term.as_deref(),
                no_term,
                resolved.as_deref(),
                bg,
                fg,
            )?;
            Ok(())
        })(),

        Some(Commands::Pr {
            branch,
            title,
            body,
            draft,
            no_push,
            worktree: is_worktree,
            by_branch,
        }) => {
            let lookup_mode = resolve_lookup_mode(is_worktree, by_branch);
            git_ops::create_pr_worktree(
                branch.as_deref(),
                !no_push,
                title.as_deref(),
                body.as_deref(),
                draft,
                lookup_mode,
            )
        }

        Some(Commands::Merge {
            branch,
            interactive,
            dry_run,
            push,
            ai_merge,
            worktree: is_worktree,
        }) => {
            let lookup_mode = if is_worktree { Some("worktree") } else { None };
            git_ops::merge_worktree(
                branch.as_deref(),
                push,
                interactive,
                dry_run,
                ai_merge,
                lookup_mode,
            )
        }

        Some(Commands::Resume {
            branch,
            term,
            bg,
            fg,
            worktree: is_worktree,
            by_branch,
        }) => {
            let lookup_mode = resolve_lookup_mode(is_worktree, by_branch);
            ai_tools::resume_worktree(branch.as_deref(), term.as_deref(), lookup_mode, bg, fg)
        }

        Some(Commands::Shell { worktree, args }) => {
            let cmd = if args.is_empty() { None } else { Some(args) };
            shell::shell_worktree(worktree.as_deref(), cmd)
        }

        Some(Commands::Delete {
            targets,
            interactive,
            dry_run,
            keep_branch,
            delete_remote,
            force,
            no_force,
            worktree: is_worktree,
            branch: is_branch,
        }) => {
            let lookup_mode = resolve_lookup_mode(is_worktree, is_branch);
            let flags = crate::operations::worktree::DeleteFlags {
                keep_branch,
                delete_remote,
                git_force: !no_force,
                allow_busy: force,
            };
            match crate::operations::delete_batch::delete_worktrees(
                targets,
                interactive,
                dry_run,
                flags,
                lookup_mode,
            ) {
                Ok(0) => Ok(()),
                Ok(code) => Err(crate::error::CwError::ExitCode(code)),
                Err(e) => Err(e),
            }
        }

        Some(Commands::Clean {
            cache,
            merged,
            older_than,
            interactive,
            dry_run,
            force,
        }) => clean::clean_worktrees(
            cache.no_cache,
            merged,
            older_than,
            interactive,
            dry_run,
            force,
        ),

        Some(Commands::Sync {
            branch,
            all,
            fetch_only,
            ai_merge,
            worktree: is_worktree,
            by_branch,
        }) => {
            let lookup_mode = resolve_lookup_mode(is_worktree, by_branch);
            worktree::sync_worktree(branch.as_deref(), all, fetch_only, ai_merge, lookup_mode)
        }

        Some(Commands::ChangeBase {
            new_base,
            branch,
            dry_run,
            interactive,
            worktree: is_worktree,
            by_branch,
        }) => {
            let lookup_mode = resolve_lookup_mode(is_worktree, by_branch);
            config_ops::change_base_branch(
                &new_base,
                branch.as_deref(),
                dry_run,
                interactive,
                lookup_mode,
            )
        }

        Some(Commands::Backup { action }) => match action {
            BackupAction::Create {
                branch,
                all,
                output,
            } => backup::backup_worktree(
                branch.as_deref(),
                all,
                output.as_deref().map(std::path::Path::new),
            ),
            BackupAction::List { branch, all } => backup::list_backups(branch.as_deref(), all),
            BackupAction::Restore { branch, path, id } => {
                backup::restore_worktree(&branch, path.as_deref(), id.as_deref())
            }
        },

        Some(Commands::Stash { action }) => match action {
            StashAction::Save { message } => stash::stash_save(message.as_deref()),
            StashAction::List => stash::stash_list(),
            StashAction::Apply {
                target_branch,
                stash: stash_ref,
            } => stash::stash_apply(&target_branch, &stash_ref),
        },

        Some(Commands::Hook { action }) => match action {
            HookAction::Add {
                event,
                command,
                id,
                description,
            } => hooks::add_hook(&event, &command, id.as_deref(), description.as_deref()).map(
                |hook_id| {
                    println!("* Added hook '{}' for {}", hook_id, event);
                },
            ),
            HookAction::Remove { event, hook_id } => hooks::remove_hook(&event, &hook_id),
            HookAction::List { event } => {
                list_hooks(event.as_deref());
                Ok(())
            }
            HookAction::Enable { event, hook_id } => {
                hooks::set_hook_enabled(&event, &hook_id, true)
            }
            HookAction::Disable { event, hook_id } => {
                hooks::set_hook_enabled(&event, &hook_id, false)
            }
            HookAction::Run { event, dry_run } => run_hooks_manual(&event, dry_run),
        },

        Some(Commands::Export { output }) => config_ops::export_config(output.as_deref()),
        Some(Commands::Import { import_file, apply }) => {
            config_ops::import_config(&import_file, apply)
        }

        Some(Commands::Scan { dir }) => global_ops::global_scan(dir.as_deref()),
        Some(Commands::Prune) => global_ops::global_prune(),
        Some(Commands::Doctor {
            session_start,
            quiet,
        }) => diagnostics::doctor(session_start, quiet),
        Some(Commands::Guard { tool_input }) => guard::run(&tool_input),
        Some(Commands::SetupClaude) => setup_claude::setup_claude(),

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
        }) => path_cmd::worktree_path(branch.as_deref(), cli.global, list_branches, interactive),

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

        Some(Commands::ConfigKeys) => {
            for (key, _desc) in config::CONFIG_KEYS {
                println!("{}", key);
            }
            Ok(())
        }

        Some(Commands::TermValues) => {
            for v in constants::all_term_values() {
                println!("{}", v);
            }
            Ok(())
        }

        Some(Commands::PresetNames) => {
            for name in constants::PRESET_NAMES {
                println!("{}", name);
            }
            Ok(())
        }

        Some(Commands::HookEvents) => {
            for evt in constants::HOOK_EVENTS {
                println!("{}", evt);
            }
            Ok(())
        }

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

fn resolve_lookup_mode(is_worktree: bool, is_branch: bool) -> Option<&'static str> {
    if is_worktree {
        Some("worktree")
    } else if is_branch {
        Some("branch")
    } else {
        None
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

fn list_hooks(event: Option<&str>) {
    let events: Vec<&str> = if let Some(e) = event {
        vec![e]
    } else {
        hooks::HOOK_EVENTS.to_vec()
    };

    let mut has_any = false;
    for evt in &events {
        let hook_list = hooks::get_hooks(evt, None);
        if hook_list.is_empty() && event.is_none() {
            continue;
        }
        if !hook_list.is_empty() {
            has_any = true;
            println!("\n{}:", evt);
            for h in &hook_list {
                let status = if h.enabled { "enabled" } else { "disabled" };
                let desc = if h.description.is_empty() {
                    String::new()
                } else {
                    format!(" - {}", h.description)
                };
                println!("  {} [{}]: {}{}", h.id, status, h.command, desc);
            }
        } else {
            println!("\n{}:", evt);
            println!("  (no hooks)");
        }
    }

    if event.is_none() && !has_any {
        println!("No hooks configured. Use 'gw hook add' to add one.");
    }
}

fn run_hooks_manual(event: &str, dry_run: bool) -> Result<()> {
    let hook_list = hooks::get_hooks(event, None);
    if hook_list.is_empty() {
        println!("No hooks configured for {}", event);
        return Ok(());
    }

    let enabled: Vec<_> = hook_list.iter().filter(|h| h.enabled).collect();
    if enabled.is_empty() {
        println!("All hooks for {} are disabled", event);
        return Ok(());
    }

    if dry_run {
        println!("Would run {} hook(s) for {}:", enabled.len(), event);
        for h in &hook_list {
            let status = if h.enabled {
                "enabled"
            } else {
                "disabled (skipped)"
            };
            let desc = if h.description.is_empty() {
                String::new()
            } else {
                format!(" - {}", h.description)
            };
            println!("  {} [{}]: {}{}", h.id, status, h.command, desc);
        }
        return Ok(());
    }

    let cwd = std::env::current_dir().unwrap_or_default();
    let context = helpers::build_hook_context("", "", &cwd, &cwd, event, "manual");

    hooks::run_hooks(event, &context, Some(&cwd), None)?;
    Ok(())
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
            .unwrap_or("your profile".to_string())
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
        let _ = std::fs::create_dir_all(cache_path.parent().unwrap_or(&home));
        if std::fs::write(&cache_path, &content).is_ok() {
            println!(
                "  {} {}",
                console::style("Created cache:").dim(),
                cache_path.display()
            );
        }
    }
}
