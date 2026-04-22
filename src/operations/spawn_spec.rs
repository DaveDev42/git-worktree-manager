//! Spawn-spec — safely launch AI tools without shell escape hazards.
//!
//! Prompts with quotes/$/backticks/newlines break when re-quoted through
//! AppleScript/wezterm/tmux send-text layers. Instead, `materialize` writes
//! argv+cwd to a temp file and returns `exec gw _spawn-ai <path>` as the
//! launcher command. `execute` reads the spec, unlinks it, chdir's, and
//! execvp's the real tool — the pane shell only ever parses ASCII.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[allow(unused_imports)]
// Used by materialize() in subsequent task
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
}
