use git_worktree_manager::resolve_prompt;
use std::io::Write;
use std::path::PathBuf;

// Convenience: most tests are not exercising the TTY path, so default to
// non-TTY (`stdin_is_tty = false`) and an unreachable! reader (since the
// inline / file paths shouldn't touch stdin).
fn no_tty() -> bool {
    false
}

#[test]
fn resolve_prompt_returns_inline_when_only_inline_set() {
    let out =
        resolve_prompt(Some("hello".to_string()), None, no_tty, || unreachable!()).unwrap();
    assert_eq!(out.as_deref(), Some("hello"));
}

#[test]
fn resolve_prompt_reads_file_contents_and_trims_trailing_newline() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p.txt");
    let mut f = std::fs::File::create(&path).unwrap();
    writeln!(f, "line1\nline2").unwrap();
    let out =
        resolve_prompt(None, Some(path.as_path()), no_tty, || unreachable!()).unwrap();
    assert_eq!(out.as_deref(), Some("line1\nline2"));
}

#[test]
fn resolve_prompt_reads_from_stdin_when_inline_is_dash() {
    // `--prompt -` reads stdin via the injected reader.
    let out = resolve_prompt(
        Some("-".to_string()),
        None,
        no_tty,
        || Ok("piped content\n".to_string()),
    )
    .unwrap();
    assert_eq!(out.as_deref(), Some("piped content"));
}

#[test]
fn resolve_prompt_returns_none_when_no_source() {
    let out = resolve_prompt(None, None, no_tty, || unreachable!()).unwrap();
    assert!(out.is_none());
}

#[test]
fn resolve_prompt_errors_when_file_missing() {
    let p = PathBuf::from("/nonexistent/definitely/not/here.txt");
    let err =
        resolve_prompt(None, Some(&p), no_tty, || unreachable!()).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("--prompt-file"),
        "expected error message to mention --prompt-file, got: {msg}"
    );
}

#[test]
fn resolve_prompt_strips_trailing_newline_from_inline() {
    let out =
        resolve_prompt(Some("hello\n".to_string()), None, no_tty, || unreachable!()).unwrap();
    assert_eq!(out.as_deref(), Some("hello"));
}

#[test]
fn resolve_prompt_strips_crlf_trailing_newline() {
    let out = resolve_prompt(
        Some("-".to_string()),
        None,
        no_tty,
        || Ok("windows content\r\n".to_string()),
    )
    .unwrap();
    assert_eq!(out.as_deref(), Some("windows content"));
}

#[test]
fn resolve_prompt_preserves_mid_content_crlf() {
    let out = resolve_prompt(
        Some("line1\r\nline2\r\n".to_string()),
        None,
        no_tty,
        || unreachable!(),
    )
    .unwrap();
    assert_eq!(out.as_deref(), Some("line1\r\nline2"));
}

#[test]
fn resolve_prompt_returns_none_for_empty_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("empty.txt");
    std::fs::File::create(&path).unwrap();
    let out =
        resolve_prompt(None, Some(path.as_path()), no_tty, || unreachable!()).unwrap();
    assert!(out.is_none(), "expected empty file to yield None");
}

#[test]
fn resolve_prompt_returns_none_for_empty_inline() {
    let out = resolve_prompt(Some(String::new()), None, no_tty, || unreachable!()).unwrap();
    assert!(out.is_none(), "expected empty inline to yield None");
}

#[test]
fn resolve_prompt_returns_none_for_whitespace_only_inline() {
    let out =
        resolve_prompt(Some("   ".to_string()), None, no_tty, || unreachable!()).unwrap();
    assert!(
        out.is_none(),
        "expected whitespace-only inline to yield None"
    );
}

#[test]
fn resolve_prompt_returns_none_for_whitespace_only_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ws.txt");
    std::fs::write(&path, "  \t\n").unwrap();
    let out =
        resolve_prompt(None, Some(path.as_path()), no_tty, || unreachable!()).unwrap();
    assert!(out.is_none(), "expected whitespace-only file to yield None");
}

#[test]
fn resolve_prompt_strips_multiple_trailing_newlines() {
    let out = resolve_prompt(
        Some("hello\n\n\n".to_string()),
        None,
        no_tty,
        || unreachable!(),
    )
    .unwrap();
    assert_eq!(out.as_deref(), Some("hello"));
}

#[test]
fn resolve_prompt_returns_none_for_whitespace_only_stdin() {
    let out = resolve_prompt(
        Some("-".to_string()),
        None,
        no_tty,
        || Ok("   \n\t\n".to_string()),
    )
    .unwrap();
    assert!(
        out.is_none(),
        "expected whitespace-only stdin to yield None"
    );
}

#[test]
fn resolve_prompt_errors_when_stdin_reader_fails() {
    let out = resolve_prompt(Some("-".to_string()), None, no_tty, || {
        Err(std::io::Error::new(
            std::io::ErrorKind::BrokenPipe,
            "pipe closed",
        ))
    });
    let err = out.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("--prompt -") || msg.contains("--prompt -:"),
        "expected error message to mention `--prompt -`, got: {msg}"
    );
}

#[test]
fn resolve_prompt_errors_when_stdin_is_tty_and_dash() {
    // Refuse to block forever when --prompt - is used at a real TTY.
    let yes_tty = || true;
    let err = resolve_prompt(Some("-".to_string()), None, yes_tty, || {
        unreachable!("reader must not run when stdin is a TTY")
    })
    .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("stdin is a terminal"),
        "expected TTY-rejection message, got: {msg}"
    );
}

#[test]
fn resolve_prompt_dash_inline_does_not_consult_tty_check_for_non_dash() {
    // Sanity: `--prompt foo` (literal "foo", not "-") must NOT invoke the
    // TTY check or the reader. The TTY check would have side effects in a
    // real implementation, so verify it stays silent.
    let panic_tty = || panic!("TTY check should not run for non-`-` inline");
    let out = resolve_prompt(
        Some("foo".to_string()),
        None,
        panic_tty,
        || unreachable!(),
    )
    .unwrap();
    assert_eq!(out.as_deref(), Some("foo"));
}
