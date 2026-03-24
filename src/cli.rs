/// CLI definitions using clap derive.
///
/// Mirrors the Typer-based CLI in src/git_worktree_manager/cli.py.
pub mod completions;
pub mod global;

use clap::{Parser, Subcommand, ValueHint};

/// Validate config key (accepts any string but provides completion hints).
fn parse_config_key(s: &str) -> Result<String, String> {
    Ok(s.to_string())
}

/// Parse duration strings like "30", "30d", "2w", "1m" into days.
fn parse_duration_days(s: &str) -> Result<u64, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("empty duration".into());
    }

    // Pure number = days
    if let Ok(n) = s.parse::<u64>() {
        return Ok(n);
    }

    let (num_str, suffix) = s.split_at(s.len() - 1);
    let n: u64 = num_str
        .parse()
        .map_err(|_| format!("invalid duration: '{}'. Use e.g. 30, 7d, 2w, 1m", s))?;

    match suffix {
        "d" => Ok(n),
        "w" => Ok(n * 7),
        "m" => Ok(n * 30),
        "y" => Ok(n * 365),
        _ => Err(format!(
            "unknown duration suffix '{}'. Use d (days), w (weeks), m (months), y (years)",
            suffix
        )),
    }
}

/// Git worktree manager CLI.
#[derive(Parser, Debug)]
#[command(
    name = "gw",
    version,
    about = "git worktree manager — AI coding assistant integration",
    long_about = None,
    arg_required_else_help = true,
)]
pub struct Cli {
    /// Run in global mode (across all registered repositories)
    #[arg(short = 'g', long = "global", global = true)]
    pub global: bool,

    /// Generate shell completions for the given shell
    #[arg(long, value_name = "SHELL")]
    pub generate_completion: Option<String>,

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
        #[arg(short = 'b', long = "base")]
        base: Option<String>,

        /// Skip AI tool launch
        #[arg(long = "no-term")]
        no_term: bool,

        /// Terminal launch method (e.g., tmux, iterm-tab, zellij)
        #[arg(short = 'T', long)]
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
        #[arg(short = 'B', long)]
        body: Option<String>,

        /// Create as draft PR
        #[arg(short, long)]
        draft: bool,

        /// Skip pushing to remote
        #[arg(long)]
        no_push: bool,

        /// Resolve target as worktree name (instead of branch)
        #[arg(short, long)]
        worktree: bool,

        /// Resolve target as branch name (instead of worktree)
        #[arg(short = 'b', long = "by-branch", conflicts_with = "worktree")]
        by_branch: bool,
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

        /// Use AI to resolve merge conflicts
        #[arg(long)]
        ai_merge: bool,

        /// Resolve target as worktree name (instead of branch)
        #[arg(short, long)]
        worktree: bool,
    },

    /// Resume AI work in a worktree
    Resume {
        /// Branch name to resume (default: current worktree)
        branch: Option<String>,

        /// Terminal launch method
        #[arg(short = 'T', long)]
        term: Option<String>,

        /// Launch AI tool in background
        #[arg(long)]
        bg: bool,

        /// Resolve target as worktree name (instead of branch)
        #[arg(short, long)]
        worktree: bool,

        /// Resolve target as branch name (instead of worktree)
        #[arg(short, long, conflicts_with = "worktree")]
        by_branch: bool,
    },

    /// Open interactive shell or execute command in a worktree
    Shell {
        /// Worktree branch to shell into
        worktree: Option<String>,

        /// Command and arguments to execute
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// Show current worktree status
    Status,

    /// Delete a worktree
    Delete {
        /// Branch name or path of worktree to delete (default: current worktree)
        target: Option<String>,

        /// Keep the branch (only remove worktree)
        #[arg(short = 'k', long)]
        keep_branch: bool,

        /// Also delete the remote branch
        #[arg(short = 'r', long)]
        delete_remote: bool,

        /// Force remove even if worktree has changes (default)
        #[arg(short, long, conflicts_with = "no_force")]
        force: bool,

        /// Don't use --force flag
        #[arg(long)]
        no_force: bool,

        /// Resolve target as worktree name (instead of branch)
        #[arg(short, long)]
        worktree: bool,

        /// Resolve target as branch name (instead of worktree)
        #[arg(short, long, conflicts_with = "worktree")]
        branch: bool,
    },

    /// List all worktrees
    #[command(alias = "ls")]
    List,

    /// Batch cleanup of worktrees
    Clean {
        /// Delete worktrees for branches already merged to base
        #[arg(long)]
        merged: bool,

        /// Delete worktrees older than duration (e.g., 7, 30d, 2w, 1m)
        #[arg(long, value_name = "DURATION", value_parser = parse_duration_days)]
        older_than: Option<u64>,

        /// Interactive selection UI
        #[arg(short, long)]
        interactive: bool,

        /// Show what would be deleted without deleting
        #[arg(long)]
        dry_run: bool,
    },

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
        #[arg(short, long)]
        summary: bool,
        /// Show changed files only
        #[arg(short, long)]
        files: bool,
    },

    /// Sync worktree with base branch
    Sync {
        /// Branch name (default: current worktree)
        branch: Option<String>,

        /// Sync all worktrees
        #[arg(long)]
        all: bool,

        /// Only fetch updates without rebasing
        #[arg(long)]
        fetch_only: bool,

        /// Use AI to resolve merge conflicts
        #[arg(long)]
        ai_merge: bool,

        /// Resolve target as worktree name (instead of branch)
        #[arg(short, long)]
        worktree: bool,

        /// Resolve target as branch name (instead of worktree)
        #[arg(short, long, conflicts_with = "worktree")]
        by_branch: bool,
    },

    /// Change base branch for a worktree
    ChangeBase {
        /// New base branch
        new_base: String,
        /// Branch name (default: current worktree)
        branch: Option<String>,

        /// Dry run (show what would happen)
        #[arg(long)]
        dry_run: bool,

        /// Interactive rebase
        #[arg(short, long)]
        interactive: bool,

        /// Resolve target as worktree name (instead of branch)
        #[arg(short, long)]
        worktree: bool,

        /// Resolve target as branch name (instead of worktree)
        #[arg(short, long, conflicts_with = "worktree")]
        by_branch: bool,
    },

    /// Configuration management
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Backup and restore worktrees
    Backup {
        #[command(subcommand)]
        action: BackupAction,
    },

    /// Stash management (worktree-aware)
    Stash {
        #[command(subcommand)]
        action: StashAction,
    },

    /// Manage lifecycle hooks
    Hook {
        #[command(subcommand)]
        action: HookAction,
    },

    /// Export worktree configuration to a file
    Export {
        /// Output file path
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Import worktree configuration from a file
    Import {
        /// Path to the configuration file to import
        import_file: String,

        /// Apply the imported configuration (default: preview only)
        #[arg(long)]
        apply: bool,
    },

    /// Scan for repositories (global mode)
    Scan {
        /// Base directory to scan (default: home directory)
        #[arg(short, long, value_hint = ValueHint::DirPath)]
        dir: Option<std::path::PathBuf>,
    },

    /// Clean up stale registry entries (global mode)
    Prune,

    /// Run diagnostics
    Doctor,

    /// Check for updates / upgrade
    Upgrade,

    /// Interactive shell integration setup
    ShellSetup,

    /// [Internal] Get worktree path for a branch
    #[command(name = "_path", hide = true)]
    Path {
        /// Branch name
        branch: Option<String>,

        /// List branch names (for tab completion)
        #[arg(long)]
        list_branches: bool,

        /// Interactive worktree selection
        #[arg(short, long)]
        interactive: bool,
    },

    /// Generate shell function for gw-cd / cw-cd
    #[command(name = "_shell-function", hide = true)]
    ShellFunction {
        /// Shell type: bash, zsh, fish, or powershell
        shell: String,
    },

    /// List config keys (for tab completion)
    #[command(name = "_config-keys", hide = true)]
    ConfigKeys,
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Show current configuration summary
    Show,
    /// List all configuration keys, values, and descriptions
    #[command(alias = "ls")]
    List,
    /// Get a configuration value
    Get {
        /// Dot-separated config key (e.g., ai_tool.command)
        #[arg(value_parser = parse_config_key)]
        key: String,
    },
    /// Set a configuration value
    Set {
        /// Dot-separated config key (e.g., ai_tool.command)
        #[arg(value_parser = parse_config_key)]
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

#[derive(Subcommand, Debug)]
pub enum BackupAction {
    /// Create backup of worktree(s) using git bundle
    Create {
        /// Branch name to backup (default: current worktree)
        branch: Option<String>,

        /// Backup all worktrees
        #[arg(long)]
        all: bool,

        /// Output directory for backups
        #[arg(short, long)]
        output: Option<String>,
    },
    /// List available backups
    List {
        /// Filter by branch name
        branch: Option<String>,

        /// Show all backups (not just current repo)
        #[arg(short, long)]
        all: bool,
    },
    /// Restore worktree from backup
    Restore {
        /// Branch name to restore
        branch: String,

        /// Custom path for restored worktree
        #[arg(short, long)]
        path: Option<String>,

        /// Backup ID (timestamp) to restore (default: latest)
        #[arg(long)]
        id: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum StashAction {
    /// Save changes in current worktree to stash
    Save {
        /// Optional message to describe the stash
        message: Option<String>,
    },
    /// List all stashes organized by worktree/branch
    List,
    /// Apply a stash to a different worktree
    Apply {
        /// Branch name of worktree to apply stash to
        target_branch: String,

        /// Stash reference (default: stash@{0})
        #[arg(short, long, default_value = "stash@{0}")]
        stash: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum HookAction {
    /// Add a new hook for an event
    Add {
        /// Hook event (e.g., worktree.post_create, merge.pre)
        event: String,
        /// Shell command to execute
        command: String,
        /// Custom hook identifier
        #[arg(long)]
        id: Option<String>,
        /// Human-readable description
        #[arg(short, long)]
        description: Option<String>,
    },
    /// Remove a hook
    Remove {
        /// Hook event
        event: String,
        /// Hook identifier to remove
        hook_id: String,
    },
    /// List all hooks
    List {
        /// Filter by event
        event: Option<String>,
    },
    /// Enable a disabled hook
    Enable {
        /// Hook event
        event: String,
        /// Hook identifier
        hook_id: String,
    },
    /// Disable a hook without removing it
    Disable {
        /// Hook event
        event: String,
        /// Hook identifier
        hook_id: String,
    },
    /// Manually run all hooks for an event
    Run {
        /// Hook event to run
        event: String,
        /// Show what would be executed without running
        #[arg(long)]
        dry_run: bool,
    },
}
