/// CLI integration tests — verify help, version, and basic arg parsing.
use assert_cmd::Command;
use clap::Parser;
use git_worktree_manager::cli::{Cli, Commands};
use predicates::prelude::*;

// Note: gw config, gw export, and gw import were removed in 1.0.
// Edit ~/.config/git-worktree-manager/config.json directly instead.

fn cw() -> Command {
    Command::cargo_bin("gw").unwrap()
}

#[test]
fn test_help() {
    cw().arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("git worktree manager"))
        .stdout(predicate::str::contains("new"))
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("resume"))
        .stdout(predicate::str::contains("delete"))
        .stdout(predicate::str::contains("doctor"))
        .stdout(predicate::str::contains("hook"))
        .stdout(predicate::str::contains("shell-setup"));
}

#[test]
fn test_version() {
    cw().arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("gw"));
}

#[test]
fn test_no_args_shows_help() {
    cw().assert()
        .failure()
        .stderr(predicate::str::contains("Usage"));
}

#[test]
fn test_shell_function_bash() {
    cw().args(["_shell-function", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("gw-cd"))
        .stdout(predicate::str::contains("_gw_cd_completion"));
}

#[test]
fn test_shell_function_fish() {
    cw().args(["_shell-function", "fish"])
        .assert()
        .success()
        .stdout(predicate::str::contains("function gw-cd"))
        .stdout(predicate::str::contains("complete -c gw-cd"));
}

#[test]
fn test_shell_function_powershell() {
    cw().args(["_shell-function", "powershell"])
        .assert()
        .success()
        .stdout(predicate::str::contains("function gw-cd"))
        .stdout(predicate::str::contains("Register-ArgumentCompleter"));
}

#[test]
fn test_shell_function_invalid() {
    cw().args(["_shell-function", "tcsh"]).assert().failure();
}

#[test]
fn test_hook_subcommands_help() {
    cw().args(["hook", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("add"))
        .stdout(predicate::str::contains("remove"))
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("enable"))
        .stdout(predicate::str::contains("disable"))
        .stdout(predicate::str::contains("run"));
}

// --- Additional CLI tests ported from test_cli.py ---

#[test]
fn test_new_help() {
    cw().args(["new", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--path"))
        .stdout(predicate::str::contains("--base"))
        .stdout(predicate::str::contains("--no-term"))
        .stdout(predicate::str::contains("--term"))
        .stdout(predicate::str::contains("--bg"))
        .stdout(predicate::str::contains("--fg"))
        .stdout(predicate::str::contains("--prompt "))
        .stdout(predicate::str::contains("--prompt-file"))
        .stdout(predicate::str::contains("--prompt-stdin"));
}

#[test]
fn test_new_bg_fg_conflict() {
    cw().args(["new", "x", "--bg", "--fg", "--no-term"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used"));
}

#[test]
fn test_delete_help() {
    cw().args(["delete", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--keep-branch"))
        .stdout(predicate::str::contains("--delete-remote"))
        .stdout(predicate::str::contains("--no-force"));
}

#[test]
fn test_delete_interactive_help_mentions_multiselect() {
    cw().args(["delete", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--interactive"));
}

#[test]
fn test_resume_help() {
    cw().args(["resume", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--term"))
        .stdout(predicate::str::contains("--bg"))
        .stdout(predicate::str::contains("--fg"));
}

#[test]
fn test_resume_bg_fg_conflict() {
    cw().args(["resume", "some-branch", "--bg", "--fg"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used"));
}

#[test]
fn test_delete_accepts_multiple_targets() {
    use clap::Parser;
    use git_worktree_manager::cli::{Cli, Commands};
    let cli = Cli::try_parse_from(["gw", "delete", "feat/a", "feat/b", "feat/c"]).expect("parses");
    let Some(Commands::Delete {
        targets,
        interactive,
        dry_run,
        ..
    }) = cli.command
    else {
        panic!("expected Delete, got {:?}", cli.command);
    };
    assert_eq!(targets, vec!["feat/a", "feat/b", "feat/c"]);
    assert!(!interactive);
    assert!(!dry_run);
}

#[test]
fn test_delete_interactive_flag_parses() {
    use clap::Parser;
    use git_worktree_manager::cli::{Cli, Commands};
    let cli = Cli::try_parse_from(["gw", "delete", "-i"]).expect("parses");
    let Some(Commands::Delete {
        targets,
        interactive,
        ..
    }) = cli.command
    else {
        panic!("expected Delete");
    };
    assert!(targets.is_empty());
    assert!(interactive);
}

#[test]
fn test_delete_dry_run_flag_parses() {
    use clap::Parser;
    use git_worktree_manager::cli::{Cli, Commands};
    let cli = Cli::try_parse_from(["gw", "delete", "a", "--dry-run"]).expect("parses");
    let Some(Commands::Delete {
        targets, dry_run, ..
    }) = cli.command
    else {
        panic!("expected Delete");
    };
    assert_eq!(targets, vec!["a"]);
    assert!(dry_run);
}

#[test]
fn test_delete_interactive_conflicts_with_positional() {
    use clap::Parser;
    use git_worktree_manager::cli::Cli;
    let err = Cli::try_parse_from(["gw", "delete", "-i", "a"]).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("cannot be used") || msg.contains("conflict"),
        "expected conflict error, got: {msg}"
    );
}

#[test]
fn test_doctor_help() {
    cw().args(["doctor", "--help"]).assert().success();
}

#[test]
fn test_list_alias_ls() {
    // "ls" should work as alias for "list"
    cw().args(["ls", "--help"]).assert().success();
}

#[test]
fn test_cw_alias_binary() {
    // The cw binary should also work
    Command::cargo_bin("cw")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("gw"));
}

#[test]
fn test_shell_function_bash_includes_cw_cd_alias() {
    cw().args(["_shell-function", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("cw-cd")) // backward compat alias
        .stdout(predicate::str::contains("gw-cd")); // primary function
}

#[test]
fn test_shell_function_fish_includes_cw_cd_alias() {
    cw().args(["_shell-function", "fish"])
        .assert()
        .success()
        .stdout(predicate::str::contains("function cw-cd"))
        .stdout(predicate::str::contains("function gw-cd"));
}

#[test]
fn test_upgrade_runs() {
    cw().args(["upgrade"]).assert().success().stdout(
        predicate::str::contains("gw") // shows version
            .or(predicate::str::contains("git-worktree-manager")),
    );
}

// --- New CLI option tests ---

#[test]
fn test_new_base_short_flag() {
    cw().args(["new", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("-b"))
        .stdout(predicate::str::contains("--base"));
}

#[test]
fn test_new_term_short_flag() {
    cw().args(["new", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("-T"))
        .stdout(predicate::str::contains("--term"));
}

#[test]
fn test_delete_short_flags() {
    cw().args(["delete", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("-k"))
        .stdout(predicate::str::contains("--keep-branch"))
        .stdout(predicate::str::contains("-r"))
        .stdout(predicate::str::contains("--delete-remote"));
}

#[test]
fn test_resume_worktree_disambiguation_flag() {
    cw().args(["resume", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--worktree"))
        .stdout(predicate::str::contains("-w"));
}

#[test]
fn test_delete_worktree_disambiguation_flag() {
    cw().args(["delete", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--worktree"))
        .stdout(predicate::str::contains("-w"));
}

#[test]
fn test_generate_completion_bash() {
    cw().args(["--generate-completion", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("complete"));
}

#[test]
fn test_generate_completion_zsh() {
    cw().args(["--generate-completion", "zsh"])
        .assert()
        .success()
        .stdout(predicate::str::contains("compdef"));
}

#[test]
fn test_generate_completion_fish() {
    cw().args(["--generate-completion", "fish"])
        .assert()
        .success()
        .stdout(predicate::str::contains("complete"));
}

#[test]
fn test_generate_completion_invalid() {
    cw().args(["--generate-completion", "tcsh"])
        .assert()
        .failure();
}

#[test]
fn new_accepts_prompt_flag() {
    let cli = Cli::try_parse_from(["gw", "new", "feat-x", "--prompt", "hello"]).expect("parses");
    let Some(Commands::New {
        prompt,
        prompt_file,
        prompt_stdin,
        ..
    }) = cli.command
    else {
        panic!("expected New variant");
    };
    assert_eq!(prompt.as_deref(), Some("hello"));
    assert!(prompt_file.is_none());
    assert!(!prompt_stdin);
}

#[test]
fn new_accepts_prompt_file_flag() {
    let cli = Cli::try_parse_from(["gw", "new", "feat-x", "--prompt-file", "/tmp/p.txt"])
        .expect("parses");
    let Some(Commands::New {
        prompt,
        prompt_file,
        prompt_stdin,
        ..
    }) = cli.command
    else {
        panic!("expected New variant");
    };
    assert!(prompt.is_none());
    assert_eq!(
        prompt_file.as_deref().and_then(|p| p.to_str()),
        Some("/tmp/p.txt")
    );
    assert!(!prompt_stdin);
}

#[test]
fn new_accepts_prompt_stdin_flag() {
    let cli = Cli::try_parse_from(["gw", "new", "feat-x", "--prompt-stdin"]).expect("parses");
    let Some(Commands::New { prompt_stdin, .. }) = cli.command else {
        panic!("expected New variant");
    };
    assert!(prompt_stdin);
}

#[test]
fn new_rejects_conflicting_prompt_sources() {
    let err = Cli::try_parse_from([
        "gw",
        "new",
        "feat-x",
        "--prompt",
        "hi",
        "--prompt-file",
        "/tmp/p.txt",
    ])
    .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("cannot be used with") || msg.contains("conflict"),
        "expected conflict error, got: {msg}"
    );
}

#[test]
fn new_rejects_prompt_and_stdin() {
    let err = Cli::try_parse_from(["gw", "new", "feat-x", "--prompt", "hi", "--prompt-stdin"])
        .unwrap_err();
    assert!(
        err.to_string().contains("cannot be used with") || err.to_string().contains("conflict")
    );
}

#[test]
fn new_rejects_file_and_stdin() {
    let err = Cli::try_parse_from([
        "gw",
        "new",
        "feat-x",
        "--prompt-file",
        "/tmp/p.txt",
        "--prompt-stdin",
    ])
    .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("cannot be used with") || msg.contains("conflict"),
        "expected conflict error, got: {msg}"
    );
}
