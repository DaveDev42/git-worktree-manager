/// Core worktree lifecycle operations.
///
use std::path::{Path, PathBuf};

use console::style;

use crate::constants::{
    default_worktree_path, format_config_key, CONFIG_KEY_BASE_BRANCH, CONFIG_KEY_BASE_PATH,
    CONFIG_KEY_INTENDED_BRANCH,
};
use crate::error::{CwError, Result};
use crate::git;
use crate::shared_files;

use super::ai_tools::LaunchOptions;
use crate::cli::EmitFormat;
use crate::messages;

/// Create a new worktree with a feature branch.
pub fn create_worktree(
    branch_name: &str,
    base_branch: Option<&str>,
    path: Option<&str>,
    initial_prompt: Option<&str>,
    launch_opts: &LaunchOptions<'_>,
    emit: EmitFormat,
) -> Result<PathBuf> {
    let repo = git::get_repo_root(None)?;

    // Validate branch name
    if !git::is_valid_branch_name(branch_name, Some(&repo)) {
        let error_msg = git::get_branch_name_error(branch_name);
        return Err(CwError::InvalidBranch(messages::invalid_branch_name(
            &error_msg,
        )));
    }

    // In json emit mode, route human-readable output to stderr so stdout
    // stays clean for the single-line JSON result.
    macro_rules! say {
        ($($arg:tt)*) => {
            if emit == EmitFormat::Json {
                eprintln!($($arg)*);
            } else {
                println!($($arg)*);
            }
        };
    }

    // Check if worktree already exists
    let existing = git::find_worktree_by_branch(&repo, branch_name)?.or(
        git::find_worktree_by_branch(&repo, &format!("refs/heads/{}", branch_name))?,
    );

    if let Some(existing_path) = existing {
        say!(
            "\n{}\nBranch '{}' already has a worktree at:\n  {}\n",
            style("! Worktree already exists").yellow().bold(),
            style(branch_name).cyan(),
            style(existing_path.display()).blue(),
        );

        if git::is_non_interactive() {
            return Err(CwError::InvalidBranch(format!(
                "Worktree for branch '{}' already exists at {}.\n\
                 Use 'gw resume {}' to continue work.",
                branch_name,
                existing_path.display(),
                branch_name,
            )));
        }

        // In interactive mode, suggest resume
        say!(
            "Use '{}' to resume work in this worktree.\n",
            style(format!("gw resume {}", branch_name)).cyan()
        );
        return Ok(existing_path);
    }

    // Determine if branch already exists
    let mut branch_already_exists = false;
    let mut is_remote_only = false;

    if git::branch_exists(branch_name, Some(&repo)) {
        say!(
            "\n{}\nBranch '{}' already exists locally but has no worktree.\n",
            style("! Branch already exists").yellow().bold(),
            style(branch_name).cyan(),
        );
        branch_already_exists = true;
    } else if git::remote_branch_exists(branch_name, Some(&repo), "origin") {
        say!(
            "\n{}\nBranch '{}' exists on remote but not locally.\n",
            style("! Remote branch found").yellow().bold(),
            style(branch_name).cyan(),
        );
        branch_already_exists = true;
        is_remote_only = true;
    }

    // Determine base branch
    let base = if let Some(b) = base_branch {
        b.to_string()
    } else {
        git::detect_default_branch(Some(&repo))
    };

    // Verify base branch
    if (!is_remote_only || base_branch.is_some()) && !git::branch_exists(&base, Some(&repo)) {
        return Err(CwError::InvalidBranch(messages::branch_not_found(&base)));
    }

    // Determine worktree path.
    // For user-supplied paths: make absolute first (relative to cwd), then
    // canonicalize to resolve symlinks. Canonicalize will fail if the path
    // does not yet exist; in that case the absolute-but-non-canonical path is
    // used — git will create the directory when it adds the worktree.
    let worktree_path = if let Some(p) = path {
        let raw = PathBuf::from(p);
        let abs = if raw.is_absolute() {
            raw
        } else {
            std::env::current_dir()
                .map_err(|e| CwError::Other(format!("cannot determine current directory: {e}")))?
                .join(raw)
        };
        abs.canonicalize().unwrap_or(abs)
    } else {
        default_worktree_path(&repo, branch_name)
    };

    say!("\n{}", style("Creating new worktree:").cyan().bold());
    say!("  Base branch: {}", style(&base).green());
    say!("  New branch:  {}", style(branch_name).green());
    say!("  Path:        {}\n", style(worktree_path.display()).blue());

    // Create parent dir
    if let Some(parent) = worktree_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    // Fetch
    let _ = git::git_command(&["fetch", "--all", "--prune"], Some(&repo), false, false);

    // Create worktree
    let wt_str = worktree_path.to_string_lossy().to_string();
    if is_remote_only {
        git::git_command(
            &[
                "worktree",
                "add",
                "-b",
                branch_name,
                &wt_str,
                &format!("origin/{}", branch_name),
            ],
            Some(&repo),
            true,
            false,
        )?;
    } else if branch_already_exists {
        git::git_command(
            &["worktree", "add", &wt_str, branch_name],
            Some(&repo),
            true,
            false,
        )?;
    } else {
        git::git_command(
            &["worktree", "add", "-b", branch_name, &wt_str, &base],
            Some(&repo),
            true,
            false,
        )?;
    }

    // Store metadata
    let bb_key = format_config_key(CONFIG_KEY_BASE_BRANCH, branch_name);
    let bp_key = format_config_key(CONFIG_KEY_BASE_PATH, branch_name);
    let ib_key = format_config_key(CONFIG_KEY_INTENDED_BRANCH, branch_name);
    git::set_config(&bb_key, &base, Some(&repo))?;
    git::set_config(&bp_key, &repo.to_string_lossy(), Some(&repo))?;
    git::set_config(&ib_key, branch_name, Some(&repo))?;

    say!(
        "{} Worktree created successfully\n",
        style("*").green().bold()
    );

    // Copy shared files
    shared_files::share_files(&repo, &worktree_path);

    // post_new fires after the worktree is on disk, so a non-zero exit
    // can't unwind the create. We propagate the error so the CLI exits
    // non-zero (the standard `Error: ...` printer in `entrypoint::run`
    // surfaces the cause); the worktree itself stays. The AI-tool launch
    // is skipped because the user signalled "this worktree isn't ready."
    crate::hooks::run_event("post_new", &worktree_path)?;

    // --emit json: write the machine-readable result to stdout and skip spawn.
    // The caller (hook or script) reads exactly this one line.
    if emit == EmitFormat::Json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "worktree_path": worktree_path.display().to_string(),
                "branch": branch_name,
                "base": base,
            }))
            .map_err(|e| CwError::Other(format!("json serialization failed: {e}")))?
        );
        return Ok(worktree_path);
    }

    // Launch AI tool in the new worktree. `-T skip|none|noop` (the
    // replacement for the old `--no-term`) is handled inside
    // spawn_in_worktree itself, so we always call through. Errors from the
    // launcher are swallowed here: the worktree is on disk and usable
    // either way, and the user can re-launch via `gw spawn`.
    let _ = super::ai_tools::spawn_in_worktree(&worktree_path, initial_prompt, launch_opts);

    Ok(worktree_path)
}

/// Outcome of attempting to delete a single worktree.
///
/// `delete_one` itself returns only `Deleted` or `Failed` today; `Skipped` is
/// carried for the batch orchestrator, which may classify an entry as skipped
/// before `delete_one` would even be called (see `rm_batch::PlanEntry`).
#[derive(Debug)]
pub enum DeletionOutcome {
    Deleted {
        branch: Option<String>,
        path: PathBuf,
    },
    Skipped {
        reason: String,
    },
    Failed {
        error: CwError,
    },
}

/// Flags that apply uniformly to every target in a batch.
#[derive(Debug, Clone, Copy)]
pub struct RmFlags {
    pub keep_branch: bool,
    pub delete_remote: bool,
    /// Passes through to `git worktree remove --force` (historical semantic).
    pub git_force: bool,
    /// Bypass the busy-detection gate.
    pub allow_busy: bool,
}

/// Per-target deletion. Assumes the caller has already resolved the target
/// and decided to proceed (no summary, no batch confirmation, no busy prompt
/// — the orchestrator handles those).
///
/// Returns an outcome describing what happened. Never prints a batch summary;
/// individual progress lines are acceptable.
pub(crate) fn delete_one(
    worktree_path: &Path,
    branch_name: Option<&str>,
    main_repo: &Path,
    flags: RmFlags,
) -> DeletionOutcome {
    // Safety: never delete the main worktree.
    let wt_resolved = git::canonicalize_or(worktree_path);
    let main_resolved = git::canonicalize_or(main_repo);
    if wt_resolved == main_resolved {
        return DeletionOutcome::Failed {
            error: CwError::Other(messages::cannot_delete_main_worktree()),
        };
    }

    // If cwd is inside worktree, move to main_repo before deletion.
    if let Ok(cwd) = std::env::current_dir() {
        let cwd_canon = cwd.canonicalize().unwrap_or(cwd);
        let wt_canon = worktree_path
            .canonicalize()
            .unwrap_or_else(|_| worktree_path.to_path_buf());
        if cwd_canon.starts_with(&wt_canon) {
            let _ = std::env::set_current_dir(main_repo);
        }
    }

    // pre_rm is advisory: a non-zero exit logs a warning but does not block
    // removal. The historical "block on non-zero" contract was dropped to
    // align with Claude Code's WorktreeRemove hook, which cannot block
    // cleanup. `--force` bypasses busy detection only — never this hook.
    if let Err(e) = crate::hooks::run_event("pre_rm", worktree_path) {
        eprintln!(
            "{} pre_rm hook failed (continuing anyway): {}",
            style("!").yellow().bold(),
            e
        );
    }

    // Remove worktree
    println!(
        "{}",
        style(messages::removing_worktree(worktree_path)).yellow()
    );
    if let Err(e) = git::remove_worktree_safe(worktree_path, main_repo, flags.git_force) {
        return DeletionOutcome::Failed { error: e };
    }
    println!("{} Worktree removed\n", style("*").green().bold());

    // Delete branch + metadata + optional remote push
    if let Some(branch) = branch_name {
        if !flags.keep_branch {
            println!(
                "{}",
                style(messages::deleting_local_branch(branch)).yellow()
            );
            let _ = git::git_command(&["branch", "-D", branch], Some(main_repo), false, false);

            let bb_key = format_config_key(CONFIG_KEY_BASE_BRANCH, branch);
            let bp_key = format_config_key(CONFIG_KEY_BASE_PATH, branch);
            let ib_key = format_config_key(CONFIG_KEY_INTENDED_BRANCH, branch);
            git::unset_config(&bb_key, Some(main_repo));
            git::unset_config(&bp_key, Some(main_repo));
            git::unset_config(&ib_key, Some(main_repo));

            println!(
                "{} Local branch and metadata removed\n",
                style("*").green().bold()
            );

            if flags.delete_remote {
                println!(
                    "{}",
                    style(messages::deleting_remote_branch(branch)).yellow()
                );
                match git::git_command(
                    &["push", "origin", &format!(":{}", branch)],
                    Some(main_repo),
                    false,
                    true,
                ) {
                    Ok(r) if r.returncode == 0 => {
                        println!("{} Remote branch deleted\n", style("*").green().bold());
                    }
                    _ => {
                        println!("{} Remote branch deletion failed\n", style("!").yellow());
                    }
                }
            }
        }
    }

    DeletionOutcome::Deleted {
        branch: branch_name.map(str::to_string),
        path: worktree_path.to_path_buf(),
    }
}

/// Delete a worktree by branch name, worktree directory name, or path.
///
/// # Parameters
///
/// * `force` — historical `git worktree remove --force` semantic. Forwarded
///   to `git::remove_worktree_safe`; controls whether git itself will remove
///   a worktree with uncommitted changes. Defaults to `true` at the CLI.
/// * `allow_busy` — bypass the gw-level busy-detection gate (lockfile +
///   process cwd scan). Wired to the explicit `--force` CLI flag on the
///   delete subcommand so users can override "worktree is in use" refusals.
///
/// These two flags are intentionally separate: the CLI `--force` is an
/// affirmative user choice to bypass the busy check, whereas the git-force
/// behaviour is a long-standing default that users rarely flip off.
pub fn delete_worktree(
    target: Option<&str>,
    keep_branch: bool,
    delete_remote: bool,
    force: bool,
    allow_busy: bool,
) -> Result<()> {
    let main_repo = git::get_main_repo_root(None)?;
    let (worktree_path, branch_name) = resolve_delete_target(target, &main_repo)?;

    // Main-repo safety guard (mirrors delete_one, but we want the error
    // surfaced up before prompting).
    let wt_resolved = git::canonicalize_or(&worktree_path);
    let main_resolved = git::canonicalize_or(&main_repo);
    if wt_resolved == main_resolved {
        return Err(CwError::Other(messages::cannot_delete_main_worktree()));
    }

    // If cwd is inside worktree, change to main repo *before* busy detection
    // so the current process itself doesn't register as a busy holder.
    // Canonicalize both sides so /var vs /private/var (macOS) and other
    // symlink skew do not hide the match.
    if let Ok(cwd) = std::env::current_dir() {
        let cwd_canon = cwd.canonicalize().unwrap_or(cwd);
        let wt_canon = worktree_path
            .canonicalize()
            .unwrap_or_else(|_| worktree_path.clone());
        if cwd_canon.starts_with(&wt_canon) {
            let _ = std::env::set_current_dir(&main_repo);
        }
    }

    let (hard, soft) = crate::operations::busy::detect_busy_tiered(&worktree_path);
    if (!hard.is_empty() || !soft.is_empty()) && !allow_busy {
        let branch_display = branch_name.clone().unwrap_or_else(|| {
            worktree_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| worktree_path.to_string_lossy().to_string())
        });
        let msg = crate::operations::busy_messages::render_refusal(&branch_display, &hard, &soft);
        eprint!("{}", msg);
        return Err(CwError::Other(format!(
            "worktree '{}' is in use; re-run with --force to override",
            branch_display
        )));
    }

    let flags = RmFlags {
        keep_branch,
        delete_remote,
        git_force: force,
        allow_busy: true, // already gated above
    };

    match delete_one(&worktree_path, branch_name.as_deref(), &main_repo, flags) {
        DeletionOutcome::Deleted { .. } => Ok(()),
        DeletionOutcome::Skipped { reason } => Err(CwError::Other(reason)),
        DeletionOutcome::Failed { error } => Err(error),
    }
}

/// Resolve delete target to (worktree_path, branch_name).
///
/// Uses strict ordered resolution: exact worktree name → exact branch → exact path.
/// When `target` is `None`, falls back to cwd as the target path.
fn resolve_delete_target(
    target: Option<&str>,
    main_repo: &Path,
) -> Result<(PathBuf, Option<String>)> {
    let target = match target {
        Some(t) => t.to_string(),
        None => std::env::current_dir()
            .map(|p| p.to_string_lossy().into_owned())
            .map_err(|e| CwError::Other(format!("cannot determine current directory: {e}")))?,
    };

    let strict = super::helpers::resolve_target_strict(main_repo, &target)?;
    Ok((strict.path, strict.branch))
}
