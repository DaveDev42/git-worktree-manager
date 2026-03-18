use std::collections::HashMap;

use clap::Parser;
use claude_worktree::cli::{BackupAction, Cli, Commands, ConfigAction, HookAction, StashAction};
use claude_worktree::config;
use claude_worktree::console as cwconsole;
use claude_worktree::hooks;
use claude_worktree::operations::{
    ai_tools, backup, clean, config_ops, diagnostics, display, git_ops, helpers, path_cmd, shell,
    stash, worktree,
};
use claude_worktree::registry;
use claude_worktree::shell_functions;
use claude_worktree::update;

fn main() {
    let cli = Cli::parse();

    // Set global mode flag
    helpers::set_global_mode(cli.global);

    let result = match cli.command {
        // Display commands
        Some(Commands::List) => display::list_worktrees(),
        Some(Commands::Status) => display::show_status(),
        Some(Commands::Tree) => display::show_tree(),
        Some(Commands::Stats) => display::show_stats(),
        Some(Commands::Diff {
            branch1,
            branch2,
            summary,
            files,
        }) => display::diff_worktrees(&branch1, &branch2, summary, files),

        // Config commands
        Some(Commands::Config { action }) => match action {
            ConfigAction::Show => config::show_config().map(|output| println!("{}", output)),
            ConfigAction::Set { key, value } => config::set_config_value(&key, &value),
            ConfigAction::UsePreset { name } => config::use_preset(&name),
            ConfigAction::ListPresets => {
                println!("{}", config::list_presets());
                Ok(())
            }
            ConfigAction::Reset => config::reset_config(),
        },

        // Core workflow
        Some(Commands::New {
            name,
            path,
            branch,
            force: _,
            no_ai,
            term,
            bg: _,
        }) => worktree::create_worktree(
            &name,
            branch.as_deref(),
            path.as_deref(),
            term.as_deref(),
            no_ai,
        )
        .map(|_| ()),

        Some(Commands::Pr {
            branch,
            title,
            body,
            draft,
            no_push,
        }) => git_ops::create_pr_worktree(
            branch.as_deref(),
            !no_push,
            title.as_deref(),
            body.as_deref(),
            draft,
        ),

        Some(Commands::Merge {
            branch,
            interactive,
            dry_run,
            push,
        }) => git_ops::merge_worktree(branch.as_deref(), push, interactive, dry_run),

        Some(Commands::Resume {
            branch,
            term,
            bg: _,
        }) => ai_tools::resume_worktree(branch.as_deref(), term.as_deref()),

        Some(Commands::Shell { worktree, args }) => {
            let cmd = if args.is_empty() { None } else { Some(args) };
            shell::shell_worktree(worktree.as_deref(), cmd)
        }

        Some(Commands::Delete {
            target,
            keep_branch,
            delete_remote,
            no_force: _,
        }) => worktree::delete_worktree(Some(&target), keep_branch, delete_remote),

        Some(Commands::Clean {
            merged,
            older_than,
            interactive,
            dry_run,
        }) => clean::clean_worktrees(merged, older_than, interactive, dry_run),

        Some(Commands::Sync {
            branch,
            all,
            fetch_only,
        }) => worktree::sync_worktree(branch.as_deref(), all, fetch_only),

        Some(Commands::ChangeBase {
            new_base,
            branch,
            dry_run,
        }) => config_ops::change_base_branch(&new_base, branch.as_deref(), dry_run),

        // Backup subcommands
        Some(Commands::Backup { action }) => match action {
            BackupAction::Create {
                branch,
                all,
                output: _,
            } => backup::backup_worktree(branch.as_deref(), all),
            BackupAction::List { branch } => backup::list_backups(branch.as_deref()),
            BackupAction::Restore { branch, path } => {
                backup::restore_worktree(&branch, path.as_deref())
            }
        },

        // Stash subcommands
        Some(Commands::Stash { action }) => match action {
            StashAction::Save { message } => stash::stash_save(message.as_deref()),
            StashAction::List => stash::stash_list(),
            StashAction::Apply {
                target_branch,
                stash: stash_ref,
            } => stash::stash_apply(&target_branch, &stash_ref),
        },

        // Hook subcommands
        Some(Commands::Hook { action }) => match action {
            HookAction::Add {
                event,
                command,
                id,
                description,
            } => {
                let hook_id =
                    hooks::add_hook(&event, &command, id.as_deref(), description.as_deref());
                match hook_id {
                    Ok(id) => {
                        println!("* Added hook '{}' for {}", id, event);
                        Ok(())
                    }
                    Err(e) => Err(e),
                }
            }
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

        // Export/Import
        Some(Commands::Export { output }) => config_ops::export_config(output.as_deref()),
        Some(Commands::Import { import_file, apply }) => {
            config_ops::import_config(&import_file, apply)
        }

        // Global management
        Some(Commands::Scan) => {
            println!("Scanning for repositories with worktrees...\n");
            let repos = registry::scan_for_repos(None, 5);
            if repos.is_empty() {
                println!("No repositories with worktrees found.\n");
            } else {
                for repo in &repos {
                    let _ = registry::register_repo(repo);
                    println!("  Registered: {}", repo.display());
                }
                println!("\n{} repository(ies) registered.\n", repos.len());
            }
            Ok(())
        }

        Some(Commands::Prune) => match registry::prune_registry() {
            Ok(removed) => {
                if removed.is_empty() {
                    println!("No stale entries found.\n");
                } else {
                    for path in &removed {
                        println!("  Removed: {}", path);
                    }
                    println!("\n{} stale entry(ies) pruned.\n", removed.len());
                }
                Ok(())
            }
            Err(e) => Err(e),
        },

        Some(Commands::Doctor) => diagnostics::doctor(),

        Some(Commands::Upgrade) => {
            update::upgrade();
            Ok(())
        }

        Some(Commands::ShellSetup) => {
            shell_setup();
            Ok(())
        }

        // Internal commands
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
            None => Err(claude_worktree::error::CwError::Config(format!(
                "Unsupported shell: {}. Use bash, zsh, or fish.",
                shell
            ))),
        },

        None => Ok(()),
    };

    if let Err(e) = result {
        cwconsole::print_error(&format!("Error: {}", e));
        std::process::exit(1);
    }
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
        if !hook_list.is_empty() || event.is_some() {
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
            } else if event.is_some() {
                println!("\n{}:", evt);
                println!("  (no hooks)");
            }
        }
    }

    if event.is_none() && !has_any {
        println!("No hooks configured. Use 'cw hook add' to add one.");
    }
}

fn run_hooks_manual(event: &str, dry_run: bool) -> claude_worktree::error::Result<()> {
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

    let mut context = HashMap::new();
    context.insert("event".into(), event.to_string());
    context.insert("operation".into(), "manual".to_string());
    context.insert("branch".into(), String::new());
    context.insert("base_branch".into(), String::new());
    let cwd = std::env::current_dir().unwrap_or_default();
    context.insert("worktree_path".into(), cwd.to_string_lossy().to_string());
    context.insert("repo_path".into(), cwd.to_string_lossy().to_string());

    hooks::run_hooks(event, &context, Some(&cwd), None)?;
    Ok(())
}

fn shell_setup() {
    // Detect shell
    let shell_env = std::env::var("SHELL").unwrap_or_default();
    let (shell_name, profile_path) = if shell_env.contains("zsh") {
        ("zsh", dirs::home_dir().map(|h| h.join(".zshrc")))
    } else if shell_env.contains("bash") {
        ("bash", dirs::home_dir().map(|h| h.join(".bashrc")))
    } else if shell_env.contains("fish") {
        (
            "fish",
            dirs::home_dir().map(|h| h.join(".config").join("fish").join("config.fish")),
        )
    } else {
        println!("Could not detect your shell automatically.\n");
        println!("Please manually add the cw-cd function to your shell:\n");
        println!("  bash/zsh: source <(cw _shell-function bash)");
        println!("  fish:     cw _shell-function fish | source");
        return;
    };

    println!("Detected shell: {}\n", shell_name);

    let line = match shell_name {
        "fish" => "cw _shell-function fish | source",
        _ => &format!("source <(cw _shell-function {})", shell_name),
    };

    // Check if already installed
    if let Some(ref path) = profile_path {
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(path) {
                if content.contains("cw _shell-function") || content.contains("cw-cd") {
                    println!("* cw-cd function is already installed!\n");
                    println!("Found in: {}", path.display());
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
    println!("\n  # claude-worktree shell integration");
    println!("  {}\n", line);

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

    if let Some(ref path) = profile_path {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let append = format!("\n# claude-worktree shell integration\n{}\n", line);

        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            Ok(mut f) => {
                use std::io::Write;
                let _ = f.write_all(append.as_bytes());
                println!("\n* Successfully added to {}", path.display());
                println!("\nNext steps:");
                println!("  1. Restart your shell or run: source {}", path.display());
                println!("  2. Try: cw-cd <branch-name>");
            }
            Err(e) => {
                println!("\nError: Failed to update {}: {}", path.display(), e);
                println!("\nTo install manually, add:");
                println!("  {}", line);
            }
        }
    }
}
