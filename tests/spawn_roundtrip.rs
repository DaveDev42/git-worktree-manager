//! End-to-end: `gw _spawn-ai <spec>` reads a spec and execvp's argv[0] with
//! argv[1..] verbatim. We point argv[0] at a platform-appropriate echo helper
//! and assert byte-for-byte prompt preservation.

use std::path::PathBuf;
use std::process::Command;

use assert_cmd::prelude::*;
use git_worktree_manager::operations::spawn_spec::SpawnSpec;
use tempfile::TempDir;

fn write_spec(dir: &TempDir, argv: Vec<String>, cwd: PathBuf) -> PathBuf {
    let spec = SpawnSpec::new(argv, cwd);
    let path = dir.path().join("spec.json");
    std::fs::write(&path, serde_json::to_vec(&spec).unwrap()).unwrap();
    path
}

#[cfg(unix)]
fn echo_program() -> String {
    "/bin/echo".into()
}

#[cfg(windows)]
fn echo_program() -> String {
    "cmd".into()
}

#[cfg(unix)]
fn build_echo_argv(prompt: &str) -> Vec<String> {
    vec![echo_program(), "-n".into(), prompt.into()]
}

#[cfg(windows)]
fn build_echo_argv(prompt: &str) -> Vec<String> {
    vec![echo_program(), "/c".into(), "echo".into(), prompt.into()]
}

fn killer_prompts() -> Vec<&'static str> {
    vec![
        r#"Fix the bug where user can "escape" quotes"#,
        r#"$(rm -rf /) — literal, not an expansion"#,
        "한글 테스트 🚀 ${PATH}",
        "multi\nline\n<<'EOF'\nnot a heredoc\nEOF\n",
        r"C:\Users\foo\bar \\path\\with\\backslashes",
        "`backtick` and 'single' and \"double\"",
    ]
}

#[test]
fn spawn_ai_preserves_prompt_bytes_exactly() {
    for prompt in killer_prompts() {
        let dir = tempfile::tempdir().unwrap();
        let argv = build_echo_argv(prompt);
        let spec_path = write_spec(&dir, argv, dir.path().to_path_buf());

        let output = Command::cargo_bin("gw")
            .unwrap()
            .arg("_spawn-ai")
            .arg(&spec_path)
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "gw _spawn-ai failed: stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );

        // Strict UTF-8 decode: if the binary ever corrupted bytes, `from_utf8`
        // would panic loudly rather than silently lossy-converting.
        let stdout = std::str::from_utf8(&output.stdout)
            .unwrap_or_else(|e| panic!("stdout not valid UTF-8 for prompt {:?}: {}", prompt, e));
        #[cfg(unix)]
        let expected = prompt.to_string();
        #[cfg(windows)]
        let expected = format!("{}\r\n", prompt);

        assert_eq!(stdout, expected, "mismatch for prompt: {:?}", prompt);

        assert!(!spec_path.exists(), "spec file should be unlinked");
    }
}
