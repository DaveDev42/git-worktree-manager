/// CLI integration tests — verify help, version, and basic arg parsing.
use assert_cmd::Command;
use predicates::prelude::*;

fn cw() -> Command {
    Command::cargo_bin("cw").unwrap()
}

#[test]
fn test_help() {
    cw().arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Claude Code"))
        .stdout(predicate::str::contains("new"))
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("merge"))
        .stdout(predicate::str::contains("pr"))
        .stdout(predicate::str::contains("resume"))
        .stdout(predicate::str::contains("delete"))
        .stdout(predicate::str::contains("doctor"))
        .stdout(predicate::str::contains("stash"))
        .stdout(predicate::str::contains("hook"))
        .stdout(predicate::str::contains("clean"))
        .stdout(predicate::str::contains("shell"))
        .stdout(predicate::str::contains("export"))
        .stdout(predicate::str::contains("import"))
        .stdout(predicate::str::contains("backup"));
}

#[test]
fn test_version() {
    cw().arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("cw"));
}

#[test]
fn test_no_args_shows_help() {
    cw().assert()
        .failure()
        .stderr(predicate::str::contains("Usage"));
}

#[test]
fn test_config_list_presets() {
    cw().args(["config", "list-presets"])
        .assert()
        .success()
        .stdout(predicate::str::contains("claude"))
        .stdout(predicate::str::contains("codex"))
        .stdout(predicate::str::contains("no-op"));
}

#[test]
fn test_shell_function_bash() {
    cw().args(["_shell-function", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("cw-cd"))
        .stdout(predicate::str::contains("_cw_cd_completion"));
}

#[test]
fn test_shell_function_fish() {
    cw().args(["_shell-function", "fish"])
        .assert()
        .success()
        .stdout(predicate::str::contains("function cw-cd"))
        .stdout(predicate::str::contains("complete -c cw-cd"));
}

#[test]
fn test_shell_function_invalid() {
    cw().args(["_shell-function", "powershell"])
        .assert()
        .failure();
}

#[test]
fn test_config_subcommands_help() {
    cw().args(["config", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("show"))
        .stdout(predicate::str::contains("set"))
        .stdout(predicate::str::contains("reset"))
        .stdout(predicate::str::contains("use-preset"))
        .stdout(predicate::str::contains("list-presets"));
}

#[test]
fn test_backup_subcommands_help() {
    cw().args(["backup", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("create"))
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("restore"));
}

#[test]
fn test_stash_subcommands_help() {
    cw().args(["stash", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("save"))
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("apply"));
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
