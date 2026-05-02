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
        .stdout(predicate::str::contains("rm"))
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
        .stdout(predicate::str::contains("--bg").not())
        .stdout(predicate::str::contains("--fg").not())
        .stdout(predicate::str::contains("--prompt "))
        .stdout(predicate::str::contains("--prompt-file"))
        .stdout(predicate::str::contains("--prompt-stdin"));
}

#[test]
fn gw_new_rejects_dropped_launch_flags() {
    // --bg, --fg were removed in Phase 6.3; clap should reject them
    cw().args(["new", "x", "--bg", "--no-term"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("unexpected argument").or(predicate::str::contains("--bg")),
        );
    cw().args(["new", "x", "--fg", "--no-term"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("unexpected argument").or(predicate::str::contains("--fg")),
        );
    // --term is restored in 1.0; --term + --no-term is a conflict (not unexpected argument)
    cw().args(["new", "x", "--term", "tmux", "--no-term"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("cannot be used with")
                .or(predicate::str::contains("conflict"))
                .or(predicate::str::contains("--term"))
                .or(predicate::str::contains("--no-term")),
        );
}

#[test]
fn test_rm_help() {
    cw().args(["rm", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--keep-branch"))
        .stdout(predicate::str::contains("--delete-remote"))
        .stdout(predicate::str::contains("--no-force"));
}

#[test]
fn test_rm_interactive_help_mentions_multiselect() {
    cw().args(["rm", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--interactive"));
}

#[test]
fn test_resume_help() {
    cw().args(["resume", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--term").not())
        .stdout(predicate::str::contains("--bg").not())
        .stdout(predicate::str::contains("--fg").not());
}

#[test]
fn gw_resume_rejects_dropped_launch_flags() {
    // --bg, --fg, --term were removed in Phase 6.3; clap should reject them
    cw().args(["resume", "some-branch", "--bg"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("unexpected argument").or(predicate::str::contains("--bg")),
        );
    cw().args(["resume", "some-branch", "--fg"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("unexpected argument").or(predicate::str::contains("--fg")),
        );
    cw().args(["resume", "some-branch", "--term", "tmux"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("unexpected argument").or(predicate::str::contains("--term")),
        );
}

#[test]
fn test_rm_accepts_multiple_targets() {
    use clap::Parser;
    use git_worktree_manager::cli::{Cli, Commands};
    let cli = Cli::try_parse_from(["gw", "rm", "feat/a", "feat/b", "feat/c"]).expect("parses");
    let Some(Commands::Rm {
        targets,
        interactive,
        dry_run,
        ..
    }) = cli.command
    else {
        panic!("expected Rm, got {:?}", cli.command);
    };
    assert_eq!(targets, vec!["feat/a", "feat/b", "feat/c"]);
    assert!(!interactive);
    assert!(!dry_run);
}

#[test]
fn test_rm_interactive_flag_parses() {
    use clap::Parser;
    use git_worktree_manager::cli::{Cli, Commands};
    let cli = Cli::try_parse_from(["gw", "rm", "-i"]).expect("parses");
    let Some(Commands::Rm {
        targets,
        interactive,
        ..
    }) = cli.command
    else {
        panic!("expected Rm");
    };
    assert!(targets.is_empty());
    assert!(interactive);
}

#[test]
fn test_rm_dry_run_flag_parses() {
    use clap::Parser;
    use git_worktree_manager::cli::{Cli, Commands};
    let cli = Cli::try_parse_from(["gw", "rm", "a", "--dry-run"]).expect("parses");
    let Some(Commands::Rm {
        targets, dry_run, ..
    }) = cli.command
    else {
        panic!("expected Rm");
    };
    assert_eq!(targets, vec!["a"]);
    assert!(dry_run);
}

#[test]
fn test_rm_interactive_conflicts_with_positional() {
    use clap::Parser;
    use git_worktree_manager::cli::Cli;
    let err = Cli::try_parse_from(["gw", "rm", "-i", "a"]).unwrap_err();
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
fn test_new_help_shows_term_flags() {
    // -T / --term were restored; verify they now appear in help
    cw().args(["new", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("-T"))
        .stdout(predicate::str::contains("--term"));
}

#[test]
fn test_rm_short_flags() {
    cw().args(["rm", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("-k"))
        .stdout(predicate::str::contains("--keep-branch"))
        .stdout(predicate::str::contains("-r"))
        .stdout(predicate::str::contains("--delete-remote"));
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

#[test]
fn new_accepts_dash_t_term() {
    let cli = Cli::try_parse_from(["gw", "new", "feat-x", "-T", "w-t"])
        .expect("parses with -T");
    match cli.command {
        Some(Commands::New { term, .. }) => assert_eq!(term.as_deref(), Some("w-t")),
        other => panic!("unexpected command variant: {:?}", other),
    }
}

#[test]
fn new_accepts_long_term() {
    let cli = Cli::try_parse_from(["gw", "new", "feat-x", "--term", "tmux:mywork"])
        .expect("parses with --term");
    match cli.command {
        Some(Commands::New { term, .. }) => {
            assert_eq!(term.as_deref(), Some("tmux:mywork"))
        }
        other => panic!("unexpected command variant: {:?}", other),
    }
}

#[test]
fn new_rejects_term_with_no_term() {
    let err = Cli::try_parse_from(["gw", "new", "feat-x", "-T", "fg", "--no-term"])
        .expect_err("clap should reject -T with --no-term");
    let msg = err.to_string();
    assert!(
        msg.contains("--no-term") || msg.contains("--term") || msg.contains("conflict"),
        "expected conflict message, got: {}",
        msg
    );
}

#[test]
fn new_rejects_dash_t_without_value() {
    let err = Cli::try_parse_from(["gw", "new", "feat-x", "-T"])
        .expect_err("clap should reject bare -T");
    assert!(err.to_string().to_lowercase().contains("value"));
}
