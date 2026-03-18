use clap::Parser;
use claude_worktree::cli::{Cli, Commands, ConfigAction};
use claude_worktree::config;
use claude_worktree::console as cwconsole;
use claude_worktree::operations::{
    ai_tools, backup, config_ops, diagnostics, display, git_ops, helpers, worktree,
};
use claude_worktree::registry;
use claude_worktree::shell_functions;
use claude_worktree::update;

fn main() {
    let cli = Cli::parse();

    // Set global mode flag
    helpers::set_global_mode(cli.global);

    let result = match cli.command {
        // Display commands (Phase 1)
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

        // Config commands (Phase 1)
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

        // Core workflow commands (Phase 2)
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

        Some(Commands::Delete {
            target,
            keep_branch,
            delete_remote,
        }) => worktree::delete_worktree(Some(&target), keep_branch, delete_remote),

        Some(Commands::Sync { branch }) => worktree::sync_worktree(branch.as_deref(), false, false),

        Some(Commands::ChangeBase { new_base, branch }) => {
            config_ops::change_base_branch(&new_base, branch.as_deref())
        }

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

        Some(Commands::Resume {
            branch,
            term,
            bg: _,
        }) => ai_tools::resume_worktree(branch.as_deref(), term.as_deref()),
        Some(Commands::Backup { branch }) => backup::backup_worktree(branch.as_deref(), false),
        Some(Commands::Restore { branch }) => backup::restore_worktree(&branch, None),
        Some(Commands::Doctor) => diagnostics::doctor(),
        Some(Commands::Upgrade) => {
            update::upgrade();
            Ok(())
        }
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
