//! `gw guard --tool-input -` reads a Claude Code hook payload on stdin and
//! decides whether to allow or block the inbound tool call.

use std::io::Write;
use std::process::{Command, Stdio};

fn run_guard_with(payload: &str) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_gw"))
        .arg("guard")
        .arg("--tool-input")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn gw guard");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(payload.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn safe_command_passes() {
    let payload = r#"{"tool_name":"Bash","tool_input":{"command":"ls -la"}}"#;
    let out = run_guard_with(payload);
    assert!(
        out.status.success(),
        "safe cmd must allow; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn risky_publish_blocked_when_cwd_unhealthy() {
    // /nonexistent/dir/xyz does not exist → unhealthy cwd → block.
    let payload =
        r#"{"tool_name":"Bash","tool_input":{"command":"git push","cwd":"/nonexistent/dir/xyz"}}"#;
    let out = run_guard_with(payload);
    assert!(
        !out.status.success(),
        "risky cmd in unhealthy cwd should block; stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("git push") || err.contains("blocking"),
        "stderr should explain the block: {err}"
    );
}

#[test]
fn risky_publish_allowed_when_cwd_healthy() {
    let tmp = tempfile::tempdir().unwrap();
    let payload = serde_json::json!({
        "tool_name": "Bash",
        "tool_input": {
            "command": "git push",
            "cwd": tmp.path().to_string_lossy(),
        }
    })
    .to_string();
    let out = run_guard_with(&payload);
    assert!(
        out.status.success(),
        "risky cmd in healthy cwd should pass; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn non_bash_tool_passes() {
    let payload = r#"{"tool_name":"Read","tool_input":{"file_path":"/tmp/x"}}"#;
    let out = run_guard_with(payload);
    assert!(out.status.success());
}

#[test]
fn missing_command_field_passes() {
    // Bash tool with no `command` field — degenerate input, allow rather
    // than block (the actual call will fail elsewhere anyway).
    let payload = r#"{"tool_name":"Bash","tool_input":{}}"#;
    let out = run_guard_with(payload);
    assert!(out.status.success());
}
