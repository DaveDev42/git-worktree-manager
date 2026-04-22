//! Spawn-spec — safely launch AI tools without shell escape hazards.
//!
//! Prompts with quotes/$/backticks/newlines break when re-quoted through
//! AppleScript/wezterm/tmux send-text layers. Instead, `materialize` writes
//! argv+cwd to a temp file and returns `exec gw _spawn-ai <path>` as the
//! launcher command. `execute` reads the spec, unlinks it, chdir's, and
//! execvp's the real tool — the pane shell only ever parses ASCII.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Result;

pub const SPEC_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SpawnSpec {
    pub version: u32,
    pub argv: Vec<String>,
    pub cwd: PathBuf,
    pub self_unlink: bool,
}

impl SpawnSpec {
    pub fn new(argv: Vec<String>, cwd: PathBuf) -> Self {
        Self {
            version: SPEC_VERSION,
            argv,
            cwd,
            self_unlink: true,
        }
    }
}

/// Write `spec` to a 0600 tempfile in the system temp dir and return
/// `(shell_line, spec_path)`. `shell_line` is safe to hand to any launcher.
pub fn materialize(spec: &SpawnSpec) -> Result<(String, PathBuf)> {
    materialize_in_dir(spec, &std::env::temp_dir())
}

/// Test seam — write into an explicit directory.
pub fn materialize_in_dir(spec: &SpawnSpec, dir: &Path) -> Result<(String, PathBuf)> {
    fs::create_dir_all(dir)?;

    // tempfile gives us a random name + O_CREAT|O_EXCL + mode 0600 on Unix.
    let named = tempfile::Builder::new()
        .prefix("gw-spawn-")
        .suffix(".json")
        .rand_bytes(16)
        .tempfile_in(dir)?;

    let json = serde_json::to_vec(spec)?;
    {
        let mut f = named.as_file();
        f.write_all(&json)?;
        f.flush()?;
    }

    // Persist — stop tempfile from auto-deleting on drop. `_spawn-ai` unlinks
    // it after reading, and the 24h sweep handles crash residue.
    let (_file, path) = named.keep().map_err(|e| e.error)?;

    let shell_line = format!("exec gw _spawn-ai {}", quote_path_for_shell(&path));
    Ok((shell_line, path))
}

/// Shell-safe rendering for a path we just created. Paths produced by
/// `tempfile_in(temp_dir())` normally contain only [A-Za-z0-9_/.-], but some
/// Windows `%TEMP%` expansions include spaces; in that case we wrap in double
/// quotes. Our own filename never contains `"`, `$`, or backslash-escaped
/// metacharacters, so double quotes are sufficient under both bash and cmd.
fn quote_path_for_shell(path: &Path) -> String {
    let s = path.to_string_lossy();
    // Backslash is NOT bare-safe: bash/zsh/tmux/wezterm interpret `\X` as an
    // escape, which would corrupt Windows paths like C:\Users\...\Temp\...
    // Any path containing `\` (or other unsafe chars) takes the quoted branch,
    // which is fine under both bash and cmd because our filename is ASCII.
    let safe = s.chars().all(|c| {
        c.is_ascii_alphanumeric() || matches!(c, '_' | '/' | '.' | '-' | ':')
    });
    if safe {
        s.into_owned()
    } else {
        format!("\"{}\"", s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_preserves_killer_prompts() {
        let killers = [
            r#"Fix the bug where user can "escape" quotes"#,
            r#"$(rm -rf /) — literal, not an expansion"#,
            "한글 테스트 🚀 ${PATH}",
            "multi\nline\n<<'EOF'\nnot a heredoc\nEOF\n",
            r"C:\Users\foo\bar \\path\\with\\backslashes",
            "`backtick` and 'single' and \"double\"",
        ];
        for prompt in killers {
            let spec = SpawnSpec::new(
                vec!["claude".into(), "--print".into(), prompt.into()],
                PathBuf::from("/tmp/wt"),
            );
            let json = serde_json::to_string(&spec).unwrap();
            let back: SpawnSpec = serde_json::from_str(&json).unwrap();
            assert_eq!(spec, back, "round-trip mismatch for: {:?}", prompt);
            assert_eq!(back.argv[2], prompt);
        }
    }

    #[test]
    fn large_prompt_round_trips() {
        let big = "x".repeat(64 * 1024);
        let spec = SpawnSpec::new(vec!["claude".into(), big.clone()], PathBuf::from("/tmp"));
        let json = serde_json::to_string(&spec).unwrap();
        let back: SpawnSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(back.argv[1], big);
    }

    #[test]
    fn materialize_writes_spec_and_returns_exec_line() {
        let dir = tempfile::tempdir().unwrap();
        let spec = SpawnSpec::new(
            vec!["/bin/echo".into(), "hello \"world\"".into()],
            dir.path().to_path_buf(),
        );
        let (shell_line, spec_path) = materialize_in_dir(&spec, dir.path()).unwrap();

        assert!(shell_line.starts_with("exec gw _spawn-ai "));
        assert!(spec_path.exists());

        let loaded: SpawnSpec =
            serde_json::from_str(&std::fs::read_to_string(&spec_path).unwrap()).unwrap();
        assert_eq!(loaded, spec);
    }

    #[test]
    fn materialize_filename_is_shell_safe() {
        let dir = tempfile::tempdir().unwrap();
        let spec = SpawnSpec::new(vec!["/bin/true".into()], dir.path().into());
        let (line, _path) = materialize_in_dir(&spec, dir.path()).unwrap();

        // "exec gw _spawn-ai " + path. path must contain only safe chars OR
        // be wrapped in double quotes. Temp dir in tests may have unsafe chars;
        // we only assert the emitted line is one of those two shapes.
        let tail = line.strip_prefix("exec gw _spawn-ai ").unwrap();
        let quoted = tail.starts_with('"') && tail.ends_with('"');
        let bare_safe = tail
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '/' | '.' | '-' | ':' | '\\'));
        assert!(quoted || bare_safe, "unsafe tail: {:?}", tail);
    }

    #[cfg(unix)]
    #[test]
    fn materialize_file_is_mode_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let spec = SpawnSpec::new(vec!["/bin/true".into()], dir.path().into());
        let (_line, path) = materialize_in_dir(&spec, dir.path()).unwrap();

        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "expected 0600, got {:o}", mode);
    }

    #[test]
    fn quote_path_for_shell_quotes_windows_backslashes() {
        use std::path::PathBuf;
        let win = PathBuf::from(r"C:\Users\me\AppData\Local\Temp\gw-spawn-abcdef0123456789.json");
        let out = super::quote_path_for_shell(&win);
        // Must be quoted — bare would let bash interpret the backslashes.
        assert!(out.starts_with('"') && out.ends_with('"'), "expected quoted, got {:?}", out);
    }

    #[test]
    fn quote_path_for_shell_bare_for_unix_paths() {
        use std::path::PathBuf;
        let unix = PathBuf::from("/tmp/gw-spawn-abcdef0123456789.json");
        let out = super::quote_path_for_shell(&unix);
        assert!(!out.starts_with('"'), "expected bare, got {:?}", out);
    }
}
