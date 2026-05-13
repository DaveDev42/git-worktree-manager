/// CLI integration tests — verify help, version, and basic arg parsing.
use assert_cmd::Command;
use clap::Parser;
use git_worktree_manager::cli::{Cli, Commands};
use predicates::prelude::*;

// Note: `gw export` / `gw import` were removed in 1.0. `gw config`
// (list/get/set/edit) was re-added; see `tests/test_config_cli.rs` for its
// CLI-level coverage.

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
        .stdout(predicate::str::contains("--term"))
        .stdout(predicate::str::contains("--bg").not())
        .stdout(predicate::str::contains("--fg").not())
        .stdout(predicate::str::contains("--prompt "))
        .stdout(predicate::str::contains("--prompt-file"));
}

#[test]
fn gw_new_forwards_unknown_flags_to_ai_tool() {
    // Trailing args are forwarded verbatim to the AI tool. Unknown flags
    // like `--bg`, `--fg`, `--no-term`, `--prompt-stdin` are NOT rejected
    // by clap — they sit in `forward_args` and would be handed to claude/
    // codex/gemini. Verify clap accepts them in that slot.
    for flag in ["--bg", "--fg", "--no-term", "--prompt-stdin"] {
        let cli = Cli::try_parse_from(["gw", "new", "feat-x", flag])
            .unwrap_or_else(|e| panic!("clap should forward {flag} into AI tool args: {e}"));
        let Some(Commands::New { forward_args, .. }) = cli.command else {
            panic!("expected New variant for flag {flag}");
        };
        assert_eq!(
            forward_args,
            vec![flag.to_string()],
            "{flag} should land in forward_args"
        );
    }
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
        .stdout(predicate::str::contains("--term")) // -T/--term restored in 1.0
        .stdout(predicate::str::contains("--bg").not())
        .stdout(predicate::str::contains("--fg").not());
}

#[test]
fn gw_resume_forwards_unknown_flags_to_ai_tool() {
    // Same forwarding behaviour as `gw new` — unknown flags ride along in
    // forward_args rather than being rejected by clap.
    for flag in ["--bg", "--fg"] {
        let cli = Cli::try_parse_from(["gw", "resume", "some-branch", flag])
            .unwrap_or_else(|e| panic!("clap should forward {flag} into AI tool args: {e}"));
        let Some(Commands::Resume { forward_args, .. }) = cli.command else {
            panic!("expected Resume variant for flag {flag}");
        };
        assert_eq!(
            forward_args,
            vec![flag.to_string()],
            "{flag} should land in resume forward_args"
        );
    }
}

/// Regression guard: clap's `trailing_var_arg=true` + `Option<positional>`
/// combo lets `--` get absorbed by the optional positional. This test pins
/// down the **observed** parser behaviour so the dispatcher's `lift_dash_target`
/// fixup stays load-bearing — if a future clap upgrade ever fixes this on
/// clap's side, the assertion below will flip and we'll know we can drop
/// the fixup.
#[test]
fn gw_spawn_dash_dash_misparses_first_token_as_target() {
    let cli = Cli::try_parse_from(["gw", "spawn", "--", "--model", "opus"]).expect("parses");
    let Some(Commands::Spawn {
        target,
        forward_args,
        ..
    }) = cli.command
    else {
        panic!("expected Spawn variant");
    };
    assert_eq!(
        target.as_deref(),
        Some("--model"),
        "documenting clap's misparse: `--` is absorbed by the optional positional"
    );
    assert_eq!(forward_args, vec!["opus".to_string()]);
}

#[test]
fn gw_resume_dash_dash_misparses_first_token_as_branch() {
    let cli = Cli::try_parse_from(["gw", "resume", "--", "--continue"]).expect("parses");
    let Some(Commands::Resume {
        branch,
        forward_args,
        ..
    }) = cli.command
    else {
        panic!("expected Resume variant");
    };
    assert_eq!(branch.as_deref(), Some("--continue"));
    assert!(forward_args.is_empty());
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
fn test_shell_function_bash_no_cw_cd_alias() {
    // Legacy cw-cd alias was removed in PR #N — verify the generated bash
    // function defines gw-cd and does NOT redefine cw-cd.
    cw().args(["_shell-function", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("gw-cd"))
        .stdout(predicate::str::contains("cw-cd").not());
}

#[test]
fn test_shell_function_fish_no_cw_cd_alias() {
    cw().args(["_shell-function", "fish"])
        .assert()
        .success()
        .stdout(predicate::str::contains("function gw-cd"))
        .stdout(predicate::str::contains("cw-cd").not());
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
fn test_new_base_long_flag() {
    // `-b` was dropped (conflicted with claude-side flags). `--base` only.
    cw().args(["new", "--help"])
        .assert()
        .success()
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
        ..
    }) = cli.command
    else {
        panic!("expected New variant");
    };
    assert_eq!(prompt.as_deref(), Some("hello"));
    assert!(prompt_file.is_none());
}

#[test]
fn new_accepts_prompt_file_flag() {
    let cli = Cli::try_parse_from(["gw", "new", "feat-x", "--prompt-file", "/tmp/p.txt"])
        .expect("parses");
    let Some(Commands::New {
        prompt,
        prompt_file,
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
}

#[test]
fn new_accepts_prompt_dash_for_stdin() {
    // `--prompt -` replaces the old `--prompt-stdin` flag. clap parses it
    // as a literal "-" — the runtime dispatch reads stdin.
    let cli =
        Cli::try_parse_from(["gw", "new", "feat-x", "--prompt", "-"]).expect("parses --prompt -");
    let Some(Commands::New { prompt, .. }) = cli.command else {
        panic!("expected New variant");
    };
    assert_eq!(prompt.as_deref(), Some("-"));
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
fn new_accepts_dash_t_term() {
    let cli = Cli::try_parse_from(["gw", "new", "feat-x", "-T", "w-t"]).expect("parses with -T");
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
fn new_term_skip_aliases_parse() {
    // `--no-term` is gone — `-T skip|none|noop` replaces it. All three
    // names must round-trip through clap.
    for alias in ["skip", "none", "noop"] {
        let cli = Cli::try_parse_from(["gw", "new", "feat-x", "-T", alias])
            .unwrap_or_else(|e| panic!("clap should accept -T {alias}: {e}"));
        match cli.command {
            Some(Commands::New { term, .. }) => assert_eq!(term.as_deref(), Some(alias)),
            other => panic!("unexpected command variant: {:?}", other),
        }
    }
}

#[test]
fn new_rejects_dash_t_without_value() {
    let err =
        Cli::try_parse_from(["gw", "new", "feat-x", "-T"]).expect_err("clap should reject bare -T");
    assert!(err.to_string().to_lowercase().contains("value"));
}

#[test]
fn resume_accepts_dash_t_term() {
    let cli = Cli::try_parse_from(["gw", "resume", "feat-x", "-T", "w-t"])
        .expect("parses resume with -T");
    match cli.command {
        Some(Commands::Resume {
            branch,
            term,
            forward_args,
            ..
        }) => {
            assert_eq!(branch.as_deref(), Some("feat-x"));
            assert_eq!(term.as_deref(), Some("w-t"));
            assert!(forward_args.is_empty());
        }
        other => panic!("unexpected command variant: {:?}", other),
    }
}

#[test]
fn resume_term_without_branch() {
    // `gw resume -T fg` (no branch) must still parse.
    let cli =
        Cli::try_parse_from(["gw", "resume", "-T", "fg"]).expect("parses resume without branch");
    match cli.command {
        Some(Commands::Resume {
            branch,
            term,
            forward_args,
            ..
        }) => {
            assert!(branch.is_none());
            assert_eq!(term.as_deref(), Some("fg"));
            assert!(forward_args.is_empty());
        }
        other => panic!("unexpected command variant: {:?}", other),
    }
}

#[test]
fn spawn_accepts_dash_t_term() {
    let cli =
        Cli::try_parse_from(["gw", "spawn", "feat-x", "-T", "w-t"]).expect("parses spawn with -T");
    match cli.command {
        Some(Commands::Spawn { target, term, .. }) => {
            assert_eq!(target.as_deref(), Some("feat-x"));
            assert_eq!(term.as_deref(), Some("w-t"));
        }
        other => panic!("unexpected command variant: {:?}", other),
    }
}

#[test]
fn spawn_term_without_target() {
    let cli =
        Cli::try_parse_from(["gw", "spawn", "-T", "fg"]).expect("parses spawn without target");
    match cli.command {
        Some(Commands::Spawn { target, term, .. }) => {
            assert!(target.is_none());
            assert_eq!(term.as_deref(), Some("fg"));
        }
        other => panic!("unexpected command variant: {:?}", other),
    }
}
