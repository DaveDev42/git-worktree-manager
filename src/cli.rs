/// CLI definitions using clap derive.
///
/// Mirrors the Typer-based CLI in src/git_worktree_manager/cli.py.
pub mod completions;

use clap::{Parser, Subcommand, ValueEnum, ValueHint};
use std::path::PathBuf;

use crate::operations::config_ops::ConfigKey;

/// Output format for `gw new --emit`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum EmitFormat {
    /// Human-readable styled output (default).
    #[default]
    Text,
    /// Single-line JSON object with worktree metadata; suppresses AI-tool launch.
    Json,
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
    /// Generate shell completions for the given shell
    #[arg(long, value_name = "SHELL", value_parser = clap::builder::PossibleValuesParser::new(["bash", "zsh", "fish", "powershell", "elvish"]))]
    pub generate_completion: Option<String>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Create new worktree for feature branch.
    ///
    /// Trailing positional args are forwarded verbatim to the underlying AI
    /// tool (claude/codex/gemini). Use `--` to disambiguate when forwarded
    /// args start with a hyphen, e.g. `gw new feat-x -- --model opus`.
    #[command(group(
        clap::ArgGroup::new("prompt_source")
            .args(["prompt", "prompt_file"])
            .multiple(false)
            .required(false)
    ))]
    New {
        /// Branch name for the new worktree
        name: String,

        /// Custom worktree path (default: ../<repo>-<branch>)
        #[arg(long, value_hint = ValueHint::DirPath)]
        path: Option<String>,

        /// Base branch to create from (default: from config)
        #[arg(long = "base")]
        base: Option<String>,

        /// Terminal launch method for THIS invocation. Overrides config
        /// and CW_LAUNCH_METHOD. Accepts canonical names (e.g.,
        /// `wezterm-tab`) or aliases (e.g., `w-t`, `w-t-b`). Supports
        /// `method:session-name` for tmux/zellij. Use `noop`, `none`, or
        /// `skip` to skip the AI tool launch entirely.
        #[arg(short = 'T', long = "term", value_name = "METHOD")]
        term: Option<String>,

        /// Initial prompt to pass to the AI tool. Use `-` to read the
        /// prompt from standard input (e.g., `--prompt -` with a heredoc
        /// or pipe).
        #[arg(long)]
        prompt: Option<String>,

        /// Read the initial prompt from a file (recommended for multi-line prompts)
        #[arg(long = "prompt-file", value_hint = ValueHint::FilePath)]
        prompt_file: Option<PathBuf>,

        /// Disable auto-forwarding of `<TOOL>_*` environment variables
        /// (e.g. `CLAUDE_*` when ai-tool is claude). Without this flag, gw
        /// snapshots the parent shell's matching env at invocation time
        /// and re-injects it inside the spawned terminal so launchers like
        /// wezterm/iterm/tmux/zellij behave like a normal child process.
        #[arg(long = "no-env-forward")]
        no_env_forward: bool,

        /// Output format for stdout. `text` (default) for human-readable styled
        /// output; `json` emits a single-line JSON object with worktree metadata
        /// and suppresses the AI-tool launch. Useful for scripts and the Claude
        /// Code `WorktreeCreate` hook integration.
        #[arg(long, value_enum, default_value_t = EmitFormat::Text)]
        emit: EmitFormat,

        /// Extra arguments forwarded verbatim to the AI tool (claude/codex/
        /// gemini). Mutually exclusive with `--prompt`/`--prompt-file`
        /// because both ultimately set the AI tool's prompt — use one or
        /// the other.
        #[arg(
            trailing_var_arg = true,
            allow_hyphen_values = true,
            value_name = "AI_TOOL_ARGS"
        )]
        forward_args: Vec<String>,
    },

    /// Resume AI work in a worktree.
    ///
    /// Trailing positional args are forwarded verbatim to the AI tool. The
    /// tool's own resume flag (`--continue`/`--resume`) is always injected
    /// alongside, so `gw resume foo -- --model opus` resumes with opus.
    Resume {
        /// Branch name, worktree name, or path to resume (default: current worktree)
        branch: Option<String>,

        /// Terminal launch method for THIS invocation. Overrides config
        /// and CW_LAUNCH_METHOD. Accepts canonical names or aliases.
        /// Supports `method:session-name` for tmux/zellij. Use `noop`,
        /// `none`, or `skip` to skip the AI tool launch.
        #[arg(short = 'T', long = "term", value_name = "METHOD")]
        term: Option<String>,

        /// Disable auto-forwarding of `<TOOL>_*` environment variables.
        #[arg(long = "no-env-forward")]
        no_env_forward: bool,

        /// Extra arguments forwarded verbatim to the AI tool (claude/codex/
        /// gemini). The tool's resume flag is still injected automatically.
        #[arg(
            trailing_var_arg = true,
            allow_hyphen_values = true,
            value_name = "AI_TOOL_ARGS"
        )]
        forward_args: Vec<String>,
    },

    /// Launch AI tool in an existing worktree (default: current).
    ///
    /// Trailing positional args are forwarded verbatim to the AI tool.
    #[command(group(
        clap::ArgGroup::new("prompt_source")
            .args(["prompt", "prompt_file"])
            .multiple(false)
            .required(false)
    ))]
    Spawn {
        /// Worktree target — exact worktree name, branch name, or path.
        /// Default: current worktree.
        target: Option<String>,

        /// Terminal launch method for THIS invocation. Overrides config
        /// and CW_LAUNCH_METHOD. Accepts canonical names or aliases.
        /// Supports `method:session-name` for tmux/zellij. Use `noop`,
        /// `none`, or `skip` to skip the AI tool launch.
        #[arg(short = 'T', long = "term", value_name = "METHOD")]
        term: Option<String>,

        /// Initial prompt to pass to the AI tool. Use `-` to read from stdin.
        #[arg(long)]
        prompt: Option<String>,

        /// Read the initial prompt from a file.
        #[arg(long = "prompt-file", value_hint = ValueHint::FilePath)]
        prompt_file: Option<PathBuf>,

        /// Disable auto-forwarding of `<TOOL>_*` environment variables.
        #[arg(long = "no-env-forward")]
        no_env_forward: bool,

        /// Extra arguments forwarded verbatim to the AI tool.
        #[arg(
            trailing_var_arg = true,
            allow_hyphen_values = true,
            value_name = "AI_TOOL_ARGS"
        )]
        forward_args: Vec<String>,
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

    /// View or edit gw configuration.
    ///
    /// Configuration lives in two scopes:
    ///   - `global`: `~/.config/git-worktree-manager/config.json`
    ///   - `repo`:   `<repo-root>/.cwconfig.json` (overrides global)
    ///
    /// `set` writes to `global` by default; pass `--repo` for a repo
    /// override. `edit` opens an interactive TUI that lets you toggle
    /// between scopes with Tab.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

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

    /// [Internal] Read a Claude Code WorktreeCreate hook payload from stdin
    /// and create a worktree via `gw new --emit json --term skip` under the hood.
    /// Prints only the worktree path (plain text, one line) to stdout.
    #[command(name = "_claude-worktree-create", hide = true)]
    ClaudeWorktreeCreate,

    /// [Internal] Read a Claude Code WorktreeRemove hook payload from stdin
    /// and run the configured `pre_rm` hook as a best-effort advisory step.
    /// Always exits 0; Claude Code owns the actual removal.
    #[command(name = "_claude-worktree-remove", hide = true)]
    ClaudeWorktreeRemove,

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

    /// Generate shell function for gw-cd
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

/// Subcommands under `gw config`.
#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// List every known key with its current value and scope.
    List,

    /// Print the resolved value of a single key (repo > global > default).
    Get {
        /// Key to look up (e.g. `ai-tool.command`).
        key: ConfigKey,
    },

    /// Set a key. Writes to global config by default; `--repo` writes to
    /// `<repo-root>/.cwconfig.json` as an override.
    Set {
        /// Key to set.
        key: ConfigKey,
        /// New value. Strings are taken as-is; booleans accept
        /// true/false/1/0/yes/no/on/off; `ai-tool.args` accepts either
        /// whitespace-separated tokens or a JSON array literal.
        value: String,
        /// Write to repo scope instead of global.
        #[arg(long)]
        repo: bool,
    },

    /// Open an interactive TUI that lets you browse/edit both scopes.
    Edit,
}
