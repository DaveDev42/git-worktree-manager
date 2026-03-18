use clap::Parser;
use claude_worktree::cli::{Cli, Commands, ConfigAction};
use claude_worktree::config;
use claude_worktree::console as cwconsole;
use claude_worktree::operations::display;

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
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

        Some(Commands::Config { action }) => match action {
            ConfigAction::Show => {
                match config::show_config() {
                    Ok(output) => {
                        println!("{}", output);
                        Ok(())
                    }
                    Err(e) => Err(e),
                }
            }
            ConfigAction::Set { key, value } => config::set_config_value(&key, &value),
            ConfigAction::UsePreset { name } => config::use_preset(&name),
            ConfigAction::ListPresets => {
                println!("{}", config::list_presets());
                Ok(())
            }
            ConfigAction::Reset => config::reset_config(),
        },

        // Phase 2+ commands — stubs for now
        Some(Commands::New { name, .. }) => {
            cwconsole::print_warning(&format!(
                "Command 'new {}' not yet implemented (Phase 2)",
                name
            ));
            Ok(())
        }
        Some(Commands::Pr { .. }) => {
            cwconsole::print_warning("Command 'pr' not yet implemented (Phase 2)");
            Ok(())
        }
        Some(Commands::Merge { .. }) => {
            cwconsole::print_warning("Command 'merge' not yet implemented (Phase 2)");
            Ok(())
        }
        Some(Commands::Resume { .. }) => {
            cwconsole::print_warning("Command 'resume' not yet implemented (Phase 2)");
            Ok(())
        }
        Some(Commands::Delete { .. }) => {
            cwconsole::print_warning("Command 'delete' not yet implemented (Phase 2)");
            Ok(())
        }
        Some(Commands::Sync { .. }) => {
            cwconsole::print_warning("Command 'sync' not yet implemented (Phase 2)");
            Ok(())
        }
        Some(Commands::ChangeBase { .. }) => {
            cwconsole::print_warning("Command 'change-base' not yet implemented (Phase 2)");
            Ok(())
        }
        Some(Commands::Backup { .. }) => {
            cwconsole::print_warning("Command 'backup' not yet implemented (Phase 2)");
            Ok(())
        }
        Some(Commands::Restore { .. }) => {
            cwconsole::print_warning("Command 'restore' not yet implemented (Phase 2)");
            Ok(())
        }
        Some(Commands::Scan) => {
            cwconsole::print_warning("Command 'scan' not yet implemented (Phase 2)");
            Ok(())
        }
        Some(Commands::Prune) => {
            cwconsole::print_warning("Command 'prune' not yet implemented (Phase 2)");
            Ok(())
        }
        Some(Commands::Doctor) => {
            cwconsole::print_warning("Command 'doctor' not yet implemented (Phase 2)");
            Ok(())
        }
        Some(Commands::Upgrade) => {
            cwconsole::print_warning("Command 'upgrade' not yet implemented (Phase 4)");
            Ok(())
        }
        Some(Commands::ShellFunction { .. }) => {
            cwconsole::print_warning("Command 'shell-function' not yet implemented (Phase 4)");
            Ok(())
        }
        None => Ok(()),
    };

    if let Err(e) = result {
        cwconsole::print_error(&format!("Error: {}", e));
        std::process::exit(1);
    }
}
