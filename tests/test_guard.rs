//! `gw guard --tool-input -` reads a Claude Code hook payload on stdin and
//! decides whether to allow or block the inbound tool call. Policy: any
//! Bash tool call from an unhealthy cwd (missing or not a directory) is
//! blocked with exit 2 and a strong abort message; healthy cwd passes.

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
fn safe_command_in_healthy_cwd_passes() {
    let tmp = tempfile::tempdir().unwrap();
    let payload = serde_json::json!({
        "tool_name": "Bash",
        "tool_input": { "command": "ls -la", "cwd": tmp.path().to_string_lossy() }
    })
    .to_string();
    let out = run_guard_with(&payload);
    assert!(
        out.status.success(),
        "safe cmd in healthy cwd must allow; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn any_bash_in_unhealthy_cwd_blocked() {
    // /nonexistent/dir/xyz does not exist → unhealthy cwd → block, even for ls.
    let payload =
        r#"{"tool_name":"Bash","tool_input":{"command":"ls -la","cwd":"/nonexistent/dir/xyz"}}"#;
    let out = run_guard_with(payload);
    assert!(
        !out.status.success(),
        "any bash in unhealthy cwd should block; stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("STOP") && err.contains("/nonexistent/dir/xyz"),
        "stderr should issue strong abort with cwd path: {err}"
    );
}

#[test]
fn risky_command_in_healthy_cwd_passes() {
    // git push is risky but if cwd is healthy we trust it.
    let tmp = tempfile::tempdir().unwrap();
    let payload = serde_json::json!({
        "tool_name": "Bash",
        "tool_input": { "command": "git push", "cwd": tmp.path().to_string_lossy() }
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
fn non_bash_tool_passes_regardless_of_cwd() {
    let payload =
        r#"{"tool_name":"Read","tool_input":{"file_path":"/tmp/x","cwd":"/nonexistent/dir/xyz"}}"#;
    let out = run_guard_with(payload);
    assert!(out.status.success());
}

#[test]
fn missing_cwd_falls_back_to_process_cwd() {
    // No cwd in payload: guard uses current_dir(). Test runner cwd is the
    // repo, which is healthy, so this should pass.
    let payload = r#"{"tool_name":"Bash","tool_input":{"command":"ls"}}"#;
    let out = run_guard_with(payload);
    assert!(
        out.status.success(),
        "missing cwd with healthy process cwd should pass; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn missing_command_field_with_healthy_cwd_passes() {
    let tmp = tempfile::tempdir().unwrap();
    let payload = serde_json::json!({
        "tool_name": "Bash",
        "tool_input": { "cwd": tmp.path().to_string_lossy() }
    })
    .to_string();
    let out = run_guard_with(&payload);
    assert!(out.status.success());
}
