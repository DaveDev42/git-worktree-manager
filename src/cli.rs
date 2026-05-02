/// CLI definitions using clap derive.
///
/// Mirrors the Typer-based CLI in src/git_worktree_manager/cli.py.
pub mod completions;

use clap::{Parser, Subcommand, ValueHint};
use std::path::PathBuf;

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
    /// Generate shell completions for the given shell
    #[arg(long, value_name = "SHELL", value_parser = clap::builder::PossibleValuesParser::new(["bash", "zsh", "fish", "powershell", "elvish"]))]
    pub generate_completion: Option<String>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Create new worktree for feature branch
    #[command(group(
        clap::ArgGroup::new("prompt_source")
            .args(["prompt", "prompt_file", "prompt_stdin"])
            .multiple(false)
            .required(false)
    ))]
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

        /// Terminal launch method for THIS invocation. Overrides config
        /// and CW_LAUNCH_METHOD. Accepts canonical names (e.g.,
        /// `wezterm-tab`) or aliases (e.g., `w-t`, `w-t-b`). Supports
        /// `method:session-name` for tmux/zellij. Mutually exclusive
        /// with `--no-term`.
        #[arg(short = 'T', long = "term", value_name = "METHOD", conflicts_with = "no_term")]
        term: Option<String>,

        /// Initial prompt to pass to the AI tool (starts interactive session with task)
        #[arg(long)]
        prompt: Option<String>,

        /// Read the initial prompt from a file (recommended for multi-line prompts)
        #[arg(long = "prompt-file", value_hint = ValueHint::FilePath)]
        prompt_file: Option<PathBuf>,

        /// Read the initial prompt from standard input
        #[arg(long = "prompt-stdin")]
        prompt_stdin: bool,
    },

    /// Resume AI work in a worktree
    Resume {
        /// Branch name, worktree name, or path to resume (default: current worktree)
        branch: Option<String>,

        /// Terminal launch method for THIS invocation. Overrides config
        /// and CW_LAUNCH_METHOD. Accepts canonical names or aliases.
        /// Supports `method:session-name` for tmux/zellij.
        #[arg(short = 'T', long = "term", value_name = "METHOD")]
        term: Option<String>,
    },

    /// Launch AI tool in an existing worktree (default: current).
    #[command(group(
        clap::ArgGroup::new("prompt_source")
            .args(["prompt", "prompt_file", "prompt_stdin"])
            .multiple(false)
            .required(false)
    ))]
    Spawn {
        /// Worktree target — exact worktree name, branch name, or path.
        /// Default: current worktree.
        target: Option<String>,

        /// Initial prompt to pass to the AI tool.
        #[arg(long)]
        prompt: Option<String>,

        /// Read the initial prompt from a file.
        #[arg(long = "prompt-file", value_hint = ValueHint::FilePath)]
        prompt_file: Option<PathBuf>,

        /// Read the initial prompt from standard input.
        #[arg(long = "prompt-stdin")]
        prompt_stdin: bool,
    },

    /// Remove one or more worktrees.
    ///
    /// With no arguments: removes the current worktree (must be inside one).
    /// With one or more positional targets: removes each of them; flags apply
    /// to every target.
    /// With `-i`: opens a multi-select UI.
    ///
    /// Exits 0 on full success, 1 if the user cancelled at the confirmation
    /// prompt or in the interactive UI, 2 if any target could not be removed
    /// (not found, busy, or an error).
    Rm {
        /// Branch names or paths of worktrees to remove.
        /// If empty and --interactive is not set, removes the current worktree.
        #[arg(conflicts_with = "interactive")]
        targets: Vec<String>,

        /// Interactive multi-select UI (mutually exclusive with positional targets)
        #[arg(short, long, conflicts_with = "targets")]
        interactive: bool,

        /// Show what would be removed without removing
        #[arg(long)]
        dry_run: bool,

        /// Keep the branch (only remove worktree)
        #[arg(short = 'k', long)]
        keep_branch: bool,

        /// Also delete the remote branch
        #[arg(short = 'r', long)]
        delete_remote: bool,

        /// Force remove: also bypasses the busy-detection gate (skips the
        /// "worktree is in use" check and removes anyway)
        #[arg(short, long, conflicts_with = "no_force")]
        force: bool,

        /// Don't use --force flag
        #[arg(long)]
        no_force: bool,
    },

    /// List all worktrees (rich, human-readable)
    List,

    /// Print all worktrees as TSV (for scripts)
    ///
    /// Columns: worktree_id, branch, status, age, repo_root, path
    Ls,

    /// Run diagnostics
    Doctor {
        /// Hook-friendly mode: emit a single-line summary and exit 0.
        #[arg(long)]
        session_start: bool,
        /// Suppress informational chatter; keep only the summary.
        #[arg(long)]
        quiet: bool,
    },

    /// Check for updates / upgrade
    Upgrade {
        /// Skip the confirmation prompt; required for non-TTY environments.
        #[arg(short, long)]
        yes: bool,
    },

    /// Install Claude Code skill for worktree task delegation
    #[command(name = "setup-claude")]
    SetupClaude,

    /// Interactive shell integration setup
    ShellSetup,

    /// Run cmd in one specific worktree.
    Exec {
        /// Worktree name, branch name, or path.
        target: String,

        /// Command and args.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true)]
        cmd: Vec<String>,
    },

    /// Run cmd in every worktree in scope.
    Run {
        /// Glob filter on branch name or worktree directory basename
        /// (e.g. 'feat-*'). Matches if either side does.
        #[arg(long)]
        only: Option<String>,
        /// Skip the main worktree.
        #[arg(long = "no-main")]
        no_main: bool,
        /// Parallel worktrees.
        #[arg(short = 'j', long, default_value = "1")]
        jobs: usize,
        /// Continue past per-worktree failures.
        #[arg(long)]
        continue_on_error: bool,
        /// Command and args.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true)]
        cmd: Vec<String>,
    },

    /// Hook helper: read a Claude Code hook payload from stdin (or a file)
    /// and decide whether to allow or block the inbound tool use. Exits 0
    /// to allow; non-zero with stderr message to block.
    Guard {
        /// Path to read the hook payload from, or "-" for stdin.
        #[arg(long, value_name = "PATH")]
        tool_input: String,
    },

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

    /// Refresh update cache (background process)
    #[command(name = "_update-cache", hide = true)]
    UpdateCache,

    /// [Internal] Print completion targets for the current repo (one per line)
    #[command(name = "_complete-targets", hide = true)]
    CompleteTargets,

    /// [Internal] Execute an AI tool spawn spec file
    #[command(name = "_spawn-ai", hide = true)]
    SpawnAi {
        /// Path to the JSON spawn spec. If omitted, resolves the most recent
        /// spec for the current worktree from `<git-dir>/gw-spawn-last.json`.
        #[arg(value_hint = ValueHint::FilePath)]
        spec: Option<PathBuf>,
    },
}
