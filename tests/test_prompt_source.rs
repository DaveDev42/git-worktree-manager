use git_worktree_manager::resolve_prompt;
use std::io::Write;
use std::path::PathBuf;

#[test]
fn resolve_prompt_returns_inline_when_only_inline_set() {
    let out = resolve_prompt(Some("hello".to_string()), None, false, || unreachable!()).unwrap();
    assert_eq!(out.as_deref(), Some("hello"));
}

#[test]
fn resolve_prompt_reads_file_contents_and_trims_trailing_newline() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p.txt");
    let mut f = std::fs::File::create(&path).unwrap();
    writeln!(f, "line1\nline2").unwrap();
    let out = resolve_prompt(None, Some(path.as_path()), false, || unreachable!()).unwrap();
    assert_eq!(out.as_deref(), Some("line1\nline2"));
}

#[test]
fn resolve_prompt_reads_from_stdin_reader() {
    let out = resolve_prompt(None, None, true, || Ok("piped content\n".to_string())).unwrap();
    assert_eq!(out.as_deref(), Some("piped content"));
}

#[test]
fn resolve_prompt_returns_none_when_no_source() {
    let out = resolve_prompt(None, None, false, || unreachable!()).unwrap();
    assert!(out.is_none());
}

#[test]
fn resolve_prompt_errors_when_file_missing() {
    let p = PathBuf::from("/nonexistent/definitely/not/here.txt");
    let err = resolve_prompt(None, Some(&p), false, || unreachable!()).unwrap_err();
    assert!(err.to_string().to_lowercase().contains("prompt"));
}

#[test]
fn resolve_prompt_strips_trailing_newline_from_inline() {
    let out = resolve_prompt(Some("hello\n".to_string()), None, false, || unreachable!()).unwrap();
    assert_eq!(out.as_deref(), Some("hello"));
}

#[test]
fn resolve_prompt_strips_crlf_trailing_newline() {
    let out = resolve_prompt(None, None, true, || Ok("windows content\r\n".to_string())).unwrap();
    assert_eq!(out.as_deref(), Some("windows content"));
}

#[test]
fn resolve_prompt_returns_none_for_empty_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("empty.txt");
    std::fs::File::create(&path).unwrap();
    let out = resolve_prompt(None, Some(path.as_path()), false, || unreachable!()).unwrap();
    assert!(out.is_none(), "expected empty file to yield None");
}

#[test]
fn resolve_prompt_returns_none_for_whitespace_only_stdin() {
    let out = resolve_prompt(None, None, true, || Ok("   \n\t\n".to_string())).unwrap();
    assert!(
        out.is_none(),
        "expected whitespace-only stdin to yield None"
    );
}

#[test]
fn resolve_prompt_errors_when_stdin_reader_fails() {
    let out = resolve_prompt(None, None, true, || {
        Err(std::io::Error::new(
            std::io::ErrorKind::BrokenPipe,
            "pipe closed",
        ))
    });
    let err = out.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("--prompt-stdin"),
        "expected error message to mention --prompt-stdin, got: {msg}"
    );
}
