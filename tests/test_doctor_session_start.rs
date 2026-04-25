//! `gw doctor --session-start --quiet` produces a single-line summary that
//! is hook-friendly. Always exits 0 so a hook failure does not abort the
//! Claude Code session start.

#[test]
fn session_start_quiet_exits_zero_in_normal_repo() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_gw"))
        .arg("doctor")
        .arg("--session-start")
        .arg("--quiet")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("run gw doctor");
    assert!(
        out.status.success(),
        "doctor --session-start should exit 0; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let line_count = stdout.lines().filter(|l| !l.trim().is_empty()).count();
    assert!(
        line_count <= 1,
        "expected at most one non-empty line, got: {stdout:?}"
    );
    assert!(
        stdout.contains("cwd=") || stdout.contains("gw:") || stdout.contains("ok="),
        "summary should contain at least one key=value pair, got: {stdout:?}"
    );
}

#[test]
fn session_start_in_non_repo_does_not_panic() {
    let tmp = tempfile::tempdir().unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_gw"))
        .arg("doctor")
        .arg("--session-start")
        .arg("--quiet")
        .current_dir(tmp.path())
        .output()
        .expect("run gw doctor");
    // Spec: doctor --session-start ALWAYS exits 0 so it cannot abort a
    // SessionStart hook. Whether it printed an error indication or just a
    // "not a repo" line is up to the impl.
    assert!(out.status.success(), "must exit 0 even outside a repo");
}
