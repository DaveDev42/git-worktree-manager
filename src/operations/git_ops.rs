/// Git operations for pull requests and merging.
///
/// Mirrors src/git_worktree_manager/operations/git_ops.py (412 lines).
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use console::style;

use crate::config;
use crate::constants::{
    format_config_key, CONFIG_KEY_BASE_BRANCH, CONFIG_KEY_BASE_PATH, CONFIG_KEY_INTENDED_BRANCH,
};
use crate::error::{CwError, Result};
use crate::git;
use crate::hooks;
use crate::registry;

use super::helpers::{build_hook_context, get_worktree_metadata, resolve_worktree_target};
use crate::messages;

/// Create a GitHub Pull Request for the worktree.
pub fn create_pr_worktree(
    target: Option<&str>,
    push: bool,
    title: Option<&str>,
    body: Option<&str>,
    draft: bool,
    lookup_mode: Option<&str>,
) -> Result<()> {
    if !git::has_command("gh") {
        return Err(CwError::Git(messages::gh_cli_not_found()));
    }

    let resolved = resolve_worktree_target(target, lookup_mode)?;
    let cwd = resolved.path;
    let feature_branch = resolved.branch;
    let (base_branch, base_path) = get_worktree_metadata(&feature_branch, &resolved.repo)?;

    println!("\n{}", style("Creating Pull Request:").cyan().bold());
    println!("  Feature:     {}", style(&feature_branch).green());
    println!("  Base:        {}", style(&base_branch).green());
    println!("  Repo:        {}\n", style(base_path.display()).blue());

    // Pre-PR hooks
    let mut hook_ctx = build_hook_context(
        &feature_branch,
        &base_branch,
        &cwd,
        &base_path,
        "pr.pre",
        "pr",
    );
    hooks::run_hooks("pr.pre", &hook_ctx, Some(&cwd), Some(&base_path))?;

    // Fetch and determine rebase target
    println!("{}", style("Fetching updates from remote...").yellow());
    let (_fetch_ok, rebase_target) = git::fetch_and_rebase_target(&base_branch, &base_path, &cwd);

    // Rebase
    println!(
        "{}",
        style(format!(
            "Rebasing {} onto {}...",
            feature_branch, rebase_target
        ))
        .yellow()
    );

    match git::git_command(&["rebase", &rebase_target], Some(&cwd), false, true) {
        Ok(r) if r.returncode == 0 => {}
        _ => {
            // Abort and report
            let conflicts = git::git_command(
                &["diff", "--name-only", "--diff-filter=U"],
                Some(&cwd),
                false,
                true,
            )
            .ok()
            .and_then(|r| {
                if r.returncode == 0 && !r.stdout.trim().is_empty() {
                    Some(r.stdout.trim().to_string())
                } else {
                    None
                }
            });

            let _ = git::git_command(&["rebase", "--abort"], Some(&cwd), false, false);

            let conflict_vec = conflicts
                .as_ref()
                .map(|c| c.lines().map(String::from).collect::<Vec<_>>());
            return Err(CwError::Rebase(messages::rebase_failed(
                &cwd.display().to_string(),
                &rebase_target,
                conflict_vec.as_deref(),
            )));
        }
    }

    println!("{} Rebase successful\n", style("*").green().bold());

    // Push
    if push {
        println!(
            "{}",
            style(format!("Pushing {} to origin...", feature_branch)).yellow()
        );
        match git::git_command(
            &["push", "-u", "origin", &feature_branch],
            Some(&cwd),
            false,
            true,
        ) {
            Ok(r) if r.returncode == 0 => {
                println!("{} Pushed to origin\n", style("*").green().bold());
            }
            Ok(r) => {
                // Try force push with lease
                match git::git_command(
                    &[
                        "push",
                        "--force-with-lease",
                        "-u",
                        "origin",
                        &feature_branch,
                    ],
                    Some(&cwd),
                    false,
                    true,
                ) {
                    Ok(r2) if r2.returncode == 0 => {
                        println!("{} Force pushed to origin\n", style("*").green().bold());
                    }
                    _ => {
                        return Err(CwError::Git(format!("Push failed: {}", r.stdout)));
                    }
                }
            }
            Err(e) => return Err(e),
        }
    }

    // Create PR
    println!("{}", style("Creating pull request...").yellow());

    let mut pr_args = vec![
        "gh".to_string(),
        "pr".to_string(),
        "create".to_string(),
        "--base".to_string(),
        base_branch.clone(),
    ];

    if let Some(t) = title {
        pr_args.extend(["--title".to_string(), t.to_string()]);
        if let Some(b) = body {
            pr_args.extend(["--body".to_string(), b.to_string()]);
        }
    } else {
        // Try AI-generated PR description
        match generate_pr_description_with_ai(&feature_branch, &base_branch, &cwd) {
            Some((ai_title, ai_body)) => {
                pr_args.extend(["--title".to_string(), ai_title]);
                pr_args.extend(["--body".to_string(), ai_body]);
            }
            None => {
                pr_args.push("--fill".to_string());
            }
        }
    }

    if draft {
        pr_args.push("--draft".to_string());
    }

    let output = Command::new(&pr_args[0])
        .args(&pr_args[1..])
        .current_dir(&cwd)
        .output()?;

    if output.status.success() {
        let pr_url = String::from_utf8_lossy(&output.stdout).trim().to_string();
        println!("{} Pull request created!\n", style("*").green().bold());
        println!("{} {}\n", style("PR URL:").bold(), pr_url);
        println!(
            "{}\n",
            style("Note: Worktree is still active. Use 'gw delete' to remove after PR is merged.")
                .dim()
        );

        // Post-PR hooks
        hook_ctx.insert("event".into(), "pr.post".into());
        hook_ctx.insert("pr_url".into(), pr_url);
        let _ = hooks::run_hooks("pr.post", &hook_ctx, Some(&cwd), Some(&base_path));
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CwError::Git(messages::pr_creation_failed(&stderr)));
    }

    Ok(())
}

/// Merge worktree: rebase, fast-forward merge, cleanup.
pub fn merge_worktree(
    target: Option<&str>,
    push: bool,
    interactive: bool,
    dry_run: bool,
    ai_merge: bool,
    lookup_mode: Option<&str>,
) -> Result<()> {
    let resolved = resolve_worktree_target(target, lookup_mode)?;
    let cwd = resolved.path;
    let feature_branch = resolved.branch;
    let (base_branch, base_path) = get_worktree_metadata(&feature_branch, &resolved.repo)?;
    let repo = &base_path;

    println!("\n{}", style("Finishing worktree:").cyan().bold());
    println!("  Feature:     {}", style(&feature_branch).green());
    println!("  Base:        {}", style(&base_branch).green());
    println!("  Repo:        {}\n", style(repo.display()).blue());

    // Pre-merge hooks
    let mut hook_ctx = build_hook_context(
        &feature_branch,
        &base_branch,
        &cwd,
        repo,
        "merge.pre",
        "merge",
    );
    if !dry_run {
        hooks::run_hooks("merge.pre", &hook_ctx, Some(&cwd), Some(repo))?;
    }

    // Dry run
    if dry_run {
        println!(
            "{}\n",
            style("DRY RUN MODE — No changes will be made")
                .yellow()
                .bold()
        );
        println!(
            "{}\n",
            style("The following operations would be performed:").bold()
        );
        println!("  1. Fetch updates from remote");
        println!("  2. Rebase {} onto {}", feature_branch, base_branch);
        println!("  3. Switch to {} in base repository", base_branch);
        println!(
            "  4. Merge {} into {} (fast-forward)",
            feature_branch, base_branch
        );
        if push {
            println!("  5. Push {} to origin", base_branch);
            println!("  6. Remove worktree at {}", cwd.display());
            println!("  7. Delete local branch {}", feature_branch);
        } else {
            println!("  5. Remove worktree at {}", cwd.display());
            println!("  6. Delete local branch {}", feature_branch);
        }
        println!("\n{}\n", style("Run without --dry-run to execute.").dim());
        return Ok(());
    }

    // Fetch and determine rebase target
    let (_fetch_ok, rebase_target) = git::fetch_and_rebase_target(&base_branch, repo, &cwd);

    // Rebase
    if interactive {
        // Interactive rebase requires a TTY — run directly via inherited stdio
        println!(
            "{}",
            style(format!(
                "Interactive rebase of {} onto {}...",
                feature_branch, rebase_target
            ))
            .yellow()
        );
        let status = Command::new("git")
            .args(["rebase", "-i", &rebase_target])
            .current_dir(&cwd)
            .status();
        match status {
            Ok(s) if s.success() => {}
            _ => {
                return Err(CwError::Rebase(messages::rebase_failed(
                    &cwd.display().to_string(),
                    &rebase_target,
                    None,
                )));
            }
        }
    } else {
        println!(
            "{}",
            style(format!(
                "Rebasing {} onto {}...",
                feature_branch, rebase_target
            ))
            .yellow()
        );

        match git::git_command(&["rebase", &rebase_target], Some(&cwd), false, true) {
            Ok(r) if r.returncode == 0 => {}
            _ => {
                if ai_merge {
                    // Try AI-assisted conflict resolution
                    let conflicts = git::git_command(
                        &["diff", "--name-only", "--diff-filter=U"],
                        Some(&cwd),
                        false,
                        true,
                    )
                    .ok()
                    .and_then(|r| {
                        if r.returncode == 0 && !r.stdout.trim().is_empty() {
                            Some(r.stdout.trim().to_string())
                        } else {
                            None
                        }
                    });

                    let _ = git::git_command(&["rebase", "--abort"], Some(&cwd), false, false);

                    let conflict_list = conflicts.as_deref().unwrap_or("(unknown)");
                    let prompt = format!(
                        "Resolve merge conflicts in this repository. The rebase of '{}' onto '{}' \
                         failed with conflicts in: {}\n\
                         Please examine the conflicted files and resolve them.",
                        feature_branch, rebase_target, conflict_list
                    );

                    println!(
                        "\n{} Launching AI to resolve conflicts...\n",
                        style("*").cyan().bold()
                    );
                    let _ = super::ai_tools::launch_ai_tool(&cwd, None, false, Some(&prompt));
                    return Ok(());
                }

                let _ = git::git_command(&["rebase", "--abort"], Some(&cwd), false, false);
                return Err(CwError::Rebase(messages::rebase_failed(
                    &cwd.display().to_string(),
                    &rebase_target,
                    None,
                )));
            }
        }
    }

    println!("{} Rebase successful\n", style("*").green().bold());

    // Verify base path
    if !base_path.exists() {
        return Err(CwError::WorktreeNotFound(messages::base_repo_not_found(
            &base_path.display().to_string(),
        )));
    }

    // Fast-forward merge
    println!(
        "{}",
        style(format!(
            "Merging {} into {}...",
            feature_branch, base_branch
        ))
        .yellow()
    );

    // Switch to base branch if needed
    let _ = git::git_command(
        &["fetch", "--all", "--prune"],
        Some(&base_path),
        false,
        false,
    );
    if let Ok(current) = git::get_current_branch(Some(&base_path)) {
        if current != base_branch {
            git::git_command(&["switch", &base_branch], Some(&base_path), true, false)?;
        }
    } else {
        git::git_command(&["switch", &base_branch], Some(&base_path), true, false)?;
    }

    match git::git_command(
        &["merge", "--ff-only", &feature_branch],
        Some(&base_path),
        false,
        true,
    ) {
        Ok(r) if r.returncode == 0 => {}
        _ => {
            return Err(CwError::Merge(messages::merge_failed(
                &base_path.display().to_string(),
                &feature_branch,
            )));
        }
    }

    println!(
        "{} Merged {} into {}\n",
        style("*").green().bold(),
        feature_branch,
        base_branch
    );

    // Push
    if push {
        println!(
            "{}",
            style(format!("Pushing {} to origin...", base_branch)).yellow()
        );
        match git::git_command(
            &["push", "origin", &base_branch],
            Some(&base_path),
            false,
            true,
        ) {
            Ok(r) if r.returncode == 0 => {
                println!("{} Pushed to origin\n", style("*").green().bold());
            }
            _ => {
                println!("{} Push failed\n", style("!").yellow());
            }
        }
    }

    // Cleanup
    println!("{}", style("Cleaning up worktree and branch...").yellow());

    let _ = std::env::set_current_dir(repo);

    git::remove_worktree_safe(&cwd, repo, true)?;
    let _ = git::git_command(&["branch", "-D", &feature_branch], Some(repo), false, false);

    // Remove metadata
    let bb_key = format_config_key(CONFIG_KEY_BASE_BRANCH, &feature_branch);
    let bp_key = format_config_key(CONFIG_KEY_BASE_PATH, &feature_branch);
    let ib_key = format_config_key(CONFIG_KEY_INTENDED_BRANCH, &feature_branch);
    git::unset_config(&bb_key, Some(repo));
    git::unset_config(&bp_key, Some(repo));
    git::unset_config(&ib_key, Some(repo));

    println!("{}\n", style("* Cleanup complete!").green().bold());

    // Post-merge hooks
    hook_ctx.insert("event".into(), "merge.post".into());
    let _ = hooks::run_hooks("merge.post", &hook_ctx, Some(repo), Some(repo));
    let _ = registry::update_last_seen(repo);

    Ok(())
}

/// Generate PR title and body using AI tool by analyzing commit history.
///
/// Returns `Some((title, body))` on success, `None` if AI is not configured or fails.
fn generate_pr_description_with_ai(
    feature_branch: &str,
    base_branch: &str,
    cwd: &Path,
) -> Option<(String, String)> {
    let ai_command = config::get_ai_tool_command().ok()?;
    if ai_command.is_empty() {
        return None;
    }

    // Get commit log for the feature branch
    let log_result = git::git_command(
        &[
            "log",
            &format!("{}..{}", base_branch, feature_branch),
            "--pretty=format:Commit: %h%nAuthor: %an%nDate: %ad%nMessage: %s%n%b%n---",
            "--date=short",
        ],
        Some(cwd),
        false,
        true,
    )
    .ok()?;

    let commits_log = log_result.stdout.trim().to_string();
    if commits_log.is_empty() {
        return None;
    }

    // Get diff stats
    let diff_stats = git::git_command(
        &[
            "diff",
            "--stat",
            &format!("{}...{}", base_branch, feature_branch),
        ],
        Some(cwd),
        false,
        true,
    )
    .ok()
    .map(|r| r.stdout.trim().to_string())
    .unwrap_or_default();

    let prompt = format!(
        "Analyze the following git commits and generate a pull request title and description.\n\n\
         Branch: {} -> {}\n\n\
         Commits:\n{}\n\n\
         Diff Statistics:\n{}\n\n\
         Please provide:\n\
         1. A concise PR title (one line, following conventional commit format if applicable)\n\
         2. A detailed PR description with:\n\
            - Summary of changes (2-3 sentences)\n\
            - Test plan (bullet points)\n\n\
         Format your response EXACTLY as:\n\
         TITLE: <your title here>\n\
         BODY:\n\
         <your body here>",
        feature_branch, base_branch, commits_log, diff_stats
    );

    // Build AI command with prompt as positional argument
    let ai_cmd = config::get_ai_tool_merge_command(&prompt).ok()?;
    if ai_cmd.is_empty() {
        return None;
    }

    println!("{}", style("Generating PR description with AI...").yellow());

    // Run AI command with 60-second timeout
    let mut child = match Command::new(&ai_cmd[0])
        .args(&ai_cmd[1..])
        .current_dir(cwd)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => {
            println!("{} Failed to start AI tool\n", style("!").yellow());
            return None;
        }
    };

    // Poll with timeout
    let deadline = std::time::Instant::now() + Duration::from_secs(crate::constants::AI_TOOL_TIMEOUT_SECS);
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) => {
                if std::time::Instant::now() > deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    println!("{} AI tool timed out\n", style("!").yellow());
                    return None;
                }
                std::thread::sleep(Duration::from_millis(crate::constants::AI_TOOL_POLL_MS));
            }
            Err(_) => return None,
        }
    };

    if !status.success() {
        println!("{} AI tool failed\n", style("!").yellow());
        return None;
    }

    // Read stdout from the completed child process
    let mut stdout_buf = String::new();
    if let Some(mut pipe) = child.stdout.take() {
        use std::io::Read;
        let _ = pipe.read_to_string(&mut stdout_buf);
    }
    let stdout = stdout_buf;
    let text = stdout.trim();

    // Parse TITLE: and BODY: from output
    match parse_ai_pr_output(text) {
        Some((t, b)) => {
            println!(
                "{} AI generated PR description\n",
                style("*").green().bold()
            );
            println!("  {} {}", style("Title:").dim(), t);
            let preview = if b.len() > 100 {
                format!("{}...", &b[..100])
            } else {
                b.clone()
            };
            println!("  {} {}\n", style("Body:").dim(), preview);
            Some((t, b))
        }
        None => {
            println!("{} Could not parse AI output\n", style("!").yellow());
            None
        }
    }
}

/// Parse AI-generated PR output into (title, body).
///
/// Expects format:
/// ```text
/// TITLE: <title>
/// BODY:
/// <body lines>
/// ```
fn parse_ai_pr_output(text: &str) -> Option<(String, String)> {
    let mut title: Option<String> = None;
    let mut body: Option<String> = None;
    let lines: Vec<&str> = text.lines().collect();

    for (i, line) in lines.iter().enumerate() {
        if let Some(t) = line.strip_prefix("TITLE:") {
            title = Some(t.trim().to_string());
        } else if line.starts_with("BODY:") {
            if i + 1 < lines.len() {
                body = Some(lines[i + 1..].join("\n").trim().to_string());
            } else {
                body = Some(String::new());
            }
            break;
        }
    }

    match (title, body) {
        (Some(t), Some(b)) if !t.is_empty() && !b.is_empty() => Some((t, b)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ai_pr_output_normal() {
        let text = "TITLE: feat: add login page\nBODY:\n## Summary\nAdded login page\n\n## Test plan\n- Manual test";
        let result = parse_ai_pr_output(text);
        assert!(result.is_some());
        let (title, body) = result.unwrap();
        assert_eq!(title, "feat: add login page");
        assert!(body.contains("## Summary"));
        assert!(body.contains("## Test plan"));
    }

    #[test]
    fn test_parse_ai_pr_output_empty() {
        assert!(parse_ai_pr_output("").is_none());
    }

    #[test]
    fn test_parse_ai_pr_output_title_only() {
        let text = "TITLE: some title";
        assert!(parse_ai_pr_output(text).is_none());
    }

    #[test]
    fn test_parse_ai_pr_output_body_only() {
        let text = "BODY:\nsome body text";
        assert!(parse_ai_pr_output(text).is_none());
    }

    #[test]
    fn test_parse_ai_pr_output_garbage() {
        let text = "This is just some random AI output\nwithout proper format";
        assert!(parse_ai_pr_output(text).is_none());
    }

    #[test]
    fn test_parse_ai_pr_output_body_at_last_line() {
        // BODY: is the last line — boundary check
        let text = "TITLE: fix: something\nBODY:";
        assert!(parse_ai_pr_output(text).is_none());
    }

    #[test]
    fn test_parse_ai_pr_output_empty_title() {
        let text = "TITLE:   \nBODY:\nsome body";
        assert!(parse_ai_pr_output(text).is_none());
    }

    #[test]
    fn test_parse_ai_pr_output_multiline_body() {
        let text = "TITLE: chore: cleanup\nBODY:\nLine 1\nLine 2\nLine 3";
        let result = parse_ai_pr_output(text).unwrap();
        assert_eq!(result.0, "chore: cleanup");
        assert_eq!(result.1, "Line 1\nLine 2\nLine 3");
    }
}
