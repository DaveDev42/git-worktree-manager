//! Spawn-spec — safely launch AI tools without shell escape hazards.
//!
//! Prompts with quotes/$/backticks/newlines break when re-quoted through
//! AppleScript/wezterm/tmux send-text layers. Instead, `materialize` writes
//! argv+cwd to a temp file and returns `gw _spawn-ai <path>` as the launcher
//! command. `execute` reads the spec, unlinks it, chdir's, and execvp's the
//! real tool — the pane shell only ever parses ASCII.
//!
//! The emitted line intentionally does NOT use `exec` so that when it is
//! fed into an already-running interactive shell (e.g. `wezterm cli
//! send-text`, iTerm AppleScript `write text`, `tmux send-keys` into a
//! session pane), the shell survives the AI tool's exit and keeps the
//! tab/pane open at its prompt. Launchers that run the line as the pane's
//! sole process via `bash -lc <line>` (tmux-window, tmux-pane-*, zellij-*)
//! are unaffected either way: their pane still closes when the AI tool
//! exits because the `bash -lc` invocation has nothing else to do.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use crate::error::{CwError, Result};

pub const SPEC_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

/// Write a non-self-unlinking copy of `spec` to `<git-dir>/gw-spawn-last.json`
/// so the user can recover from a corrupted launcher line by running
/// `gw _spawn-ai` (no args) inside the worktree.
///
/// `git_dir` is the path returned by `git rev-parse --git-dir` — for the
/// main checkout this is `<repo>/.git/`, for linked worktrees it is
/// `<repo>/.git/worktrees/<name>/`. Either way the file lives per-worktree.
///
/// Best-effort: any IO error is returned; callers may choose to swallow it
/// because the launcher itself still works without the persistent copy.
fn write_last_to_git_dir(spec: &SpawnSpec, git_dir: &Path) -> Result<()> {
    let mut persistent = spec.clone();
    // Critical: persistent copy must survive `execute()`'s unlink so the
    // user can re-run `gw _spawn-ai` repeatedly after a corruption event.
    persistent.self_unlink = false;
    let path = git_dir.join("gw-spawn-last.json");
    let json = serde_json::to_vec(&persistent)?;
    fs::write(&path, json)?;
    Ok(())
}

/// Look up the git-dir for `cwd` via `git rev-parse --git-dir`. Returns the
/// absolute path to the `.git` directory (or `.git/worktrees/<name>/` for
/// linked worktrees).
fn git_dir_for(cwd: &Path) -> Result<PathBuf> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .current_dir(cwd)
        .output()
        .map_err(|e| CwError::Other(format!("spawn-ai: git rev-parse failed: {}", e)))?;
    if !output.status.success() {
        return Err(CwError::Other(format!(
            "spawn-ai: not a git worktree: {}",
            cwd.display()
        )));
    }
    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let p = PathBuf::from(&raw);
    let absolute = if p.is_absolute() { p } else { cwd.join(p) };
    Ok(absolute)
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

    // Best-effort persistent copy for `gw _spawn-ai` (no args) recovery.
    // If `spec.cwd` is not a git worktree (unusual — AI tools always run
    // inside one) the failure is swallowed: the launcher still works via
    // the random temp path.
    if let Ok(git_dir) = git_dir_for(&spec.cwd) {
        let _ = write_last_to_git_dir(spec, &git_dir);
    }

    let shell_line = format!("gw _spawn-ai {}", quote_path_for_shell(&path));
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
    let safe = s
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '/' | '.' | '-' | ':'));
    if safe {
        s.into_owned()
    } else {
        format!("\"{}\"", s)
    }
}

/// Parse a spec file, rejecting unsupported versions and empty argv.
/// All errors are prefixed with `spawn-ai:` so the entrypoint can print
/// them verbatim without duplicating the prefix.
pub fn read_spec(path: &Path) -> Result<SpawnSpec> {
    let bytes = fs::read(path)
        .map_err(|e| CwError::Other(format!("spawn-ai: read {} failed: {}", path.display(), e)))?;
    let spec: SpawnSpec = serde_json::from_slice(&bytes)
        .map_err(|e| CwError::Other(format!("spawn-ai: parse {} failed: {}", path.display(), e)))?;
    if spec.version != SPEC_VERSION {
        return Err(CwError::Other(format!(
            "spawn-ai: unsupported spawn spec version: {} (expected {})",
            spec.version, SPEC_VERSION
        )));
    }
    if spec.argv.is_empty() {
        return Err(CwError::Other("spawn-ai: spawn spec has empty argv".into()));
    }
    Ok(spec)
}

/// Execute a spawn spec. Never returns to the caller:
/// - Unix: `execvp` replaces the current process. On exec failure we print a
///   diagnostic to stderr and `exit(127)` ("command not found" convention).
/// - Windows: spawns a child, waits, and exits with the child's code. Spawn
///   failures also exit 127.
///
/// The `Result<()>` return type exists only so the caller can surface
/// pre-spawn errors (spec read, parse, chdir) through the normal error path;
/// once `execvp`/spawn is attempted, the process exits directly.
pub fn execute(spec_path: &Path) -> Result<()> {
    let spec = read_spec(spec_path)?;

    if spec.self_unlink {
        // Best-effort — proceed even if unlink fails (e.g. already gone).
        let _ = fs::remove_file(spec_path);
    }

    std::env::set_current_dir(&spec.cwd).map_err(|e| {
        CwError::Other(format!(
            "spawn-ai: chdir to {} failed: {}",
            spec.cwd.display(),
            e
        ))
    })?;

    let program = &spec.argv[0];
    let args = &spec.argv[1..];

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = std::process::Command::new(program).args(args).exec();
        // exec only returns on failure.
        eprintln!("spawn-ai: exec {} failed: {}", program, err);
        std::process::exit(127);
    }

    #[cfg(windows)]
    {
        // Exit directly with the child's code to mirror Unix execvp semantics
        // as closely as we can on Windows (no true process replacement).
        let status = match std::process::Command::new(program).args(args).status() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("spawn-ai: spawn {} failed: {}", program, e);
                std::process::exit(127);
            }
        };
        let code = status.code().unwrap_or(1);
        std::process::exit(code);
    }
}

/// Resolve the last persisted spec path for the current working directory's
/// worktree. Reads `<git-dir>/gw-spawn-last.json`. Errors are prefixed with
/// `spawn-ai:` so the entrypoint can print them verbatim.
pub fn resolve_last_for_cwd() -> Result<PathBuf> {
    Err(CwError::Other(
        "spawn-ai: resolve_last_for_cwd not implemented yet".into(),
    ))
}

/// Best-effort removal of stale `gw-spawn-*.json` temp files from the system
/// temp directory. Intended to run once at `gw` startup. All errors are
/// swallowed — this is a safety net, not a correctness mechanism.
pub fn sweep_stale() {
    sweep_stale_in(&std::env::temp_dir(), Duration::from_secs(24 * 3600));
}

fn sweep_stale_in(dir: &Path, max_age: Duration) {
    let entries = match fs::read_dir(dir) {
        Ok(it) => it,
        Err(_) => return,
    };
    let now = SystemTime::now();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if !name_str.starts_with("gw-spawn-") || !name_str.ends_with(".json") {
            continue;
        }
        // symlink_metadata + is_file narrows the TOCTOU window: we refuse to
        // follow symlinks or delete directories that happen to match the name.
        let metadata = match fs::symlink_metadata(entry.path()) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if !metadata.is_file() {
            continue;
        }
        let mtime = match metadata.modified() {
            Ok(t) => t,
            Err(_) => continue,
        };
        if now.duration_since(mtime).unwrap_or_default() > max_age {
            let _ = fs::remove_file(entry.path());
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

    #[test]
    fn materialize_writes_spec_and_returns_shell_line() {
        let dir = tempfile::tempdir().unwrap();
        let spec = SpawnSpec::new(
            vec!["/bin/echo".into(), "hello \"world\"".into()],
            dir.path().to_path_buf(),
        );
        let (shell_line, spec_path) = materialize_in_dir(&spec, dir.path()).unwrap();

        // No `exec` — the shell must survive after the AI tool exits so the
        // terminal tab/pane stays open (e.g. WezTerm tab keeps the zsh prompt
        // after claude quits).
        assert!(shell_line.starts_with("gw _spawn-ai "));
        // Strictly redundant with the prefix check above, but kept as a
        // self-documenting guard: if someone ever changes the emitted prefix
        // string in the future, the `exec` ban is load-bearing for tab
        // lifetime and must not silently regress.
        assert!(
            !shell_line.starts_with("exec "),
            "shell_line must not use exec: {:?}",
            shell_line
        );
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

        // "gw _spawn-ai " + path. path must contain only safe chars OR
        // be wrapped in double quotes. Temp dir in tests may have unsafe chars;
        // we only assert the emitted line is one of those two shapes.
        let tail = line.strip_prefix("gw _spawn-ai ").unwrap();
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
        assert!(
            out.starts_with('"') && out.ends_with('"'),
            "expected quoted, got {:?}",
            out
        );
    }

    #[test]
    fn quote_path_for_shell_bare_for_unix_paths() {
        use std::path::PathBuf;
        let unix = PathBuf::from("/tmp/gw-spawn-abcdef0123456789.json");
        let out = super::quote_path_for_shell(&unix);
        assert!(!out.starts_with('"'), "expected bare, got {:?}", out);
    }

    #[test]
    fn read_spec_rejects_wrong_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.json");
        std::fs::write(
            &path,
            r#"{"version":999,"argv":["x"],"cwd":"/","self_unlink":false}"#,
        )
        .unwrap();
        let err = read_spec(&path).unwrap_err();
        assert!(format!("{err}").contains("unsupported spawn spec version"));
    }

    #[test]
    fn read_spec_rejects_empty_argv() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.json");
        std::fs::write(
            &path,
            r#"{"version":1,"argv":[],"cwd":"/","self_unlink":false}"#,
        )
        .unwrap();
        let err = read_spec(&path).unwrap_err();
        assert!(format!("{err}").contains("empty argv"));
    }

    #[test]
    fn read_spec_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let spec = SpawnSpec::new(
            vec!["/bin/echo".into(), "hi".into()],
            dir.path().to_path_buf(),
        );
        let path = dir.path().join("ok.json");
        std::fs::write(&path, serde_json::to_vec(&spec).unwrap()).unwrap();
        let loaded = read_spec(&path).unwrap();
        assert_eq!(loaded, spec);
    }

    #[test]
    fn materialize_writes_last_copy_into_git_dir() {
        let dir = tempfile::tempdir().unwrap();
        let worktree = dir.path();
        let status = std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(worktree)
            .status()
            .unwrap();
        assert!(status.success());
        let git_dir = worktree.join(".git");

        let spec = SpawnSpec::new(
            vec!["claude".into(), "--print".into(), "hello".into()],
            worktree.to_path_buf(),
        );

        let temp = tempfile::tempdir().unwrap();
        let (_line, _path) = materialize_in_dir(&spec, temp.path()).unwrap();

        let last = git_dir.join("gw-spawn-last.json");
        assert!(
            last.exists(),
            "expected persistent copy at {}",
            last.display()
        );

        let loaded: SpawnSpec =
            serde_json::from_str(&std::fs::read_to_string(&last).unwrap()).unwrap();
        assert!(
            !loaded.self_unlink,
            "persistent copy must have self_unlink=false"
        );
        assert_eq!(loaded.argv, spec.argv);
        assert_eq!(loaded.cwd, spec.cwd);
    }

    #[test]
    fn sweep_stale_removes_old_spec_files_only() {
        use std::time::{Duration, SystemTime};
        let dir = tempfile::tempdir().unwrap();

        // Old spec file — mtime far in the past.
        let old = dir.path().join("gw-spawn-old.json");
        std::fs::write(&old, "{}").unwrap();
        let past = SystemTime::now() - Duration::from_secs(48 * 3600);
        filetime::set_file_mtime(&old, filetime::FileTime::from_system_time(past)).unwrap();

        // Recent spec file — should survive.
        let recent = dir.path().join("gw-spawn-recent.json");
        std::fs::write(&recent, "{}").unwrap();

        // Unrelated file — should survive regardless of age.
        let unrelated = dir.path().join("something-else.json");
        std::fs::write(&unrelated, "{}").unwrap();
        filetime::set_file_mtime(&unrelated, filetime::FileTime::from_system_time(past)).unwrap();

        sweep_stale_in(dir.path(), Duration::from_secs(24 * 3600));

        assert!(!old.exists(), "old gw-spawn file should be removed");
        assert!(recent.exists(), "recent gw-spawn file should remain");
        assert!(unrelated.exists(), "unrelated file should be untouched");
    }
}
