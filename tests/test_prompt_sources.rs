use git_worktree_manager::prompt_source::resolve_prompt;
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
