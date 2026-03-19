/// CLI integration tests — verify help, version, and basic arg parsing.
use assert_cmd::Command;
use predicates::prelude::*;

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
        .stdout(predicate::str::contains("gw"));
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

// --- Additional CLI tests ported from test_cli.py ---

#[test]
fn test_new_help() {
    cw().args(["new", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--path"))
        .stdout(predicate::str::contains("--branch"))
        .stdout(predicate::str::contains("--no-ai"))
        .stdout(predicate::str::contains("--term"));
}

#[test]
fn test_pr_help() {
    cw().args(["pr", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--title"))
        .stdout(predicate::str::contains("--body"))
        .stdout(predicate::str::contains("--draft"))
        .stdout(predicate::str::contains("--no-push"));
}

#[test]
fn test_merge_help() {
    cw().args(["merge", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--interactive"))
        .stdout(predicate::str::contains("--dry-run"))
        .stdout(predicate::str::contains("--push"));
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
fn test_sync_help() {
    cw().args(["sync", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--all"))
        .stdout(predicate::str::contains("--fetch-only"));
}

#[test]
fn test_clean_help() {
    cw().args(["clean", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--merged"))
        .stdout(predicate::str::contains("--older-than"))
        .stdout(predicate::str::contains("--interactive"))
        .stdout(predicate::str::contains("--dry-run"));
}

#[test]
fn test_resume_help() {
    cw().args(["resume", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--term"));
}

#[test]
fn test_change_base_help() {
    cw().args(["change-base", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--dry-run"));
}

#[test]
fn test_export_help() {
    cw().args(["export", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--output"));
}

#[test]
fn test_import_help() {
    cw().args(["import", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--apply"));
}

#[test]
fn test_shell_help() {
    cw().args(["shell", "--help"]).assert().success();
}

#[test]
fn test_doctor_help() {
    cw().args(["doctor", "--help"]).assert().success();
}

#[test]
fn test_tree_help() {
    cw().args(["tree", "--help"]).assert().success();
}

#[test]
fn test_stats_help() {
    cw().args(["stats", "--help"]).assert().success();
}

#[test]
fn test_diff_help() {
    cw().args(["diff", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--summary"))
        .stdout(predicate::str::contains("--files"));
}

#[test]
fn test_global_flag() {
    // -g flag should be accepted (even without proper context)
    cw().args(["-g", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--global"));
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

#[test]
fn test_config_show_runs() {
    cw().args(["config", "show"])
        .assert()
        .success()
        .stdout(predicate::str::contains("AI Tool:"));
}
