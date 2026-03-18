/// CLI definitions using clap derive.
///
/// Mirrors the Typer-based CLI in src/claude_worktree/cli.py.
pub mod completions;
pub mod global;

use clap::{Parser, Subcommand, ValueHint};

/// Claude Code x git worktree helper CLI.
#[derive(Parser, Debug)]
#[command(
    name = "cw",
    version,
    about = "Claude Code × git worktree helper CLI",
    long_about = None,
    arg_required_else_help = true,
)]
pub struct Cli {
    /// Run in global mode (across all registered repositories)
    #[arg(short = 'g', long = "global", global = true)]
    pub global: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Create new worktree for feature branch
    New {
        /// Branch name for the new worktree
        name: String,

        /// Custom worktree path (default: ../<repo>-<branch>)
        #[arg(short, long, value_hint = ValueHint::DirPath)]
        path: Option<String>,

        /// Base branch to create from (default: from config)
        #[arg(short, long)]
        branch: Option<String>,

        /// Force creation even if branch exists
        #[arg(short, long)]
        force: bool,

        /// Skip AI tool launch
        #[arg(long)]
        no_ai: bool,

        /// Terminal launch method (e.g., tmux, iterm-tab, zellij)
        #[arg(long)]
        term: Option<String>,

        /// Launch AI tool in background
        #[arg(long)]
        bg: bool,
    },

    /// Create GitHub Pull Request from worktree
    Pr {
        /// Branch name (default: current worktree branch)
        branch: Option<String>,

        /// PR title
        #[arg(short, long)]
        title: Option<String>,

        /// PR body
        #[arg(short, long)]
        body: Option<String>,

        /// Create as draft PR
        #[arg(short, long)]
        draft: bool,

        /// Skip pushing to remote
        #[arg(long)]
        no_push: bool,
    },

    /// Merge feature branch into base branch
    Merge {
        /// Branch name (default: current worktree branch)
        branch: Option<String>,

        /// Interactive rebase
        #[arg(short, long)]
        interactive: bool,

        /// Dry run (show what would happen)
        #[arg(long)]
        dry_run: bool,

        /// Push to remote after merge
        #[arg(long)]
        push: bool,
    },

    /// Resume AI work in a worktree
    Resume {
        /// Branch name to resume (default: current worktree)
        branch: Option<String>,

        /// Terminal launch method
        #[arg(long)]
        term: Option<String>,

        /// Launch AI tool in background
        #[arg(long)]
        bg: bool,
    },

    /// Show current worktree status
    Status,

    /// Delete a worktree
    Delete {
        /// Branch name or path of worktree to delete
        target: String,

        /// Keep the branch (only remove worktree)
        #[arg(long)]
        keep_branch: bool,

        /// Also delete the remote branch
        #[arg(long)]
        delete_remote: bool,
    },

    /// List all worktrees
    #[command(alias = "ls")]
    List,

    /// Display worktree hierarchy as a tree
    Tree,

    /// Show worktree statistics
    Stats,

    /// Compare two branches
    Diff {
        /// First branch
        branch1: String,
        /// Second branch
        branch2: String,
        /// Show statistics only
        #[arg(long)]
        summary: bool,
        /// Show changed files only
        #[arg(long)]
        files: bool,
    },

    /// Sync worktree with base branch
    Sync {
        /// Branch name (default: current worktree)
        branch: Option<String>,
    },

    /// Change base branch for a worktree
    ChangeBase {
        /// New base branch
        new_base: String,
        /// Branch name (default: current worktree)
        branch: Option<String>,
    },

    /// Configuration management
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Backup worktree state
    Backup {
        /// Branch name (default: current worktree)
        branch: Option<String>,
    },

    /// Restore worktree from backup
    Restore {
        /// Branch name to restore
        branch: String,
    },

    /// Scan for repositories (global mode)
    Scan,

    /// Clean up stale registry entries (global mode)
    Prune,

    /// Run diagnostics
    Doctor,

    /// Check for updates / upgrade
    Upgrade,

    /// Generate shell function for cw-cd
    #[command(name = "_shell-function")]
    ShellFunction {
        /// Shell type: bash, zsh, or fish
        shell: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Show current configuration
    Show,
    /// Set a configuration value
    Set {
        /// Dot-separated config key (e.g., git.default_base_branch)
        key: String,
        /// Value to set
        value: String,
    },
    /// Use a predefined AI tool preset
    UsePreset {
        /// Preset name (e.g., claude, codex, no-op)
        name: String,
    },
    /// List available presets
    ListPresets,
    /// Reset configuration to defaults
    Reset,
}
