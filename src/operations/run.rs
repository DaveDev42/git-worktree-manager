//! `gw run <cmd>` — fan a command out across all worktrees in scope.

use std::io::Write;
use std::path::Path;
use std::process::Command;

use crate::error::Result;
use crate::scope;

pub fn run_in_scope(
    cwd: &Path,
    cmd: &[String],
    only: Option<&str>,
    no_main: bool,
    jobs: usize,
    continue_on_error: bool,
) -> Result<i32> {
    let mut stdout = std::io::stdout().lock();
    run_in_scope_to_writer(cwd, cmd, only, no_main, jobs, continue_on_error, &mut stdout)
}

pub fn run_in_scope_to_writer<W: Write>(
    cwd: &Path,
    cmd: &[String],
    only: Option<&str>,
    no_main: bool,
    jobs: usize,
    continue_on_error: bool,
    out: &mut W,
) -> Result<i32> {
    let scope = scope::discover_scope(cwd)?;
    let mut targets: Vec<&scope::ScopedWorktree> = scope
        .worktrees()
        .iter()
        .filter(|w| !no_main || !w.is_main)
        .filter(|w| match only {
            Some(g) => glob_match(g, &w.name),
            None => true,
        })
        .collect();
    targets.sort_by_key(|w| (!w.is_main, w.name.clone())); // main first

    let jobs = jobs.max(1);
    let mut last_failure: i32 = 0;
    let mut i = 0;
    while i < targets.len() {
        let batch_end = (i + jobs).min(targets.len());

        // Spawn each child in this batch on its own thread, capturing
        // (target_index, captured_output, exit_code).
        let handles: Vec<std::thread::JoinHandle<Result<(usize, String, i32)>>> = (i..batch_end)
            .map(|k| {
                let w = targets[k];
                let name = w.name.clone();
                let path = w.path.clone();
                let cmd_v: Vec<String> = cmd.to_vec();
                std::thread::spawn(move || {
                    let (s, code) = run_capture_one(&name, &path, &cmd_v)?;
                    Ok((k, s, code))
                })
            })
            .collect();

        // Collect, then sort by target index so output is emitted in target order
        // (NOT completion order). This preserves the "main first, then alphabetical"
        // ordering established by the earlier sort_by_key.
        let mut results: Vec<(usize, String, i32)> = handles
            .into_iter()
            .map(|h| h.join().expect("worker panicked"))
            .collect::<Result<Vec<_>>>()?;
        results.sort_by_key(|(k, _, _)| *k);

        for (_, s, code) in results {
            let _ = out.write_all(s.as_bytes());
            if code != 0 {
                last_failure = code;
                if !continue_on_error {
                    return Ok(last_failure);
                }
            }
        }
        i = batch_end;
    }
    Ok(last_failure)
}

pub(crate) fn run_capture_one(
    name: &str,
    path: &Path,
    cmd: &[String],
) -> Result<(String, i32)> {
    let prefix = format!("[{}] ", name);
    let mut child = Command::new(&cmd[0])
        .args(&cmd[1..])
        .current_dir(path)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");

    let prefix_a = prefix.clone();
    let h1 = std::thread::spawn(move || pipe_with_prefix(stdout, &prefix_a));
    let prefix_b = prefix.clone();
    let h2 = std::thread::spawn(move || pipe_with_prefix(stderr, &prefix_b));

    let so = h1.join().unwrap_or_default();
    let se = h2.join().unwrap_or_default();
    let mut combined = so;
    combined.push_str(&se);

    let status = child.wait()?;
    Ok((combined, status.code().unwrap_or(1)))
}

fn pipe_with_prefix<R: std::io::Read>(reader: R, prefix: &str) -> String {
    use std::io::BufRead;
    let buf = std::io::BufReader::new(reader);
    let mut out = String::new();
    for line in buf.lines().map_while(|l| l.ok()) {
        out.push_str(prefix);
        out.push_str(&line);
        out.push('\n');
    }
    out
}

fn glob_match(pattern: &str, name: &str) -> bool {
    // Empty pattern matches only empty name.
    if pattern.is_empty() {
        return name.is_empty();
    }
    // Minimal glob: supports '*' anywhere. No '?', no character classes.
    let parts: Vec<&str> = pattern.split('*').collect();
    let mut idx = 0usize;
    let last = parts.len() - 1;
    if !pattern.starts_with('*') && !name.starts_with(parts[0]) {
        return false;
    }
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        // For the last segment when the pattern is not '*'-terminated, use
        // rfind so that the trailing literal is anchored to end-of-string
        // rather than matching the first occurrence in the remaining slice.
        let pos = if i == last && !pattern.ends_with('*') {
            name[idx..].rfind(part)
        } else {
            name[idx..].find(part)
        };
        match pos {
            Some(p) => idx += p + part.len(),
            None => return false,
        }
        if i == last && !pattern.ends_with('*') && idx != name.len() {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod glob_tests {
    use super::glob_match;

    #[test]
    fn empty_pattern_matches_only_empty() {
        assert!(glob_match("", ""));
        assert!(!glob_match("", "anything"));
    }

    #[test]
    fn star_matches_anything() {
        assert!(glob_match("*", ""));
        assert!(glob_match("*", "anything"));
    }

    #[test]
    fn literal_matches_exactly() {
        assert!(glob_match("foo", "foo"));
        assert!(!glob_match("foo", "foobar"));
        assert!(!glob_match("foo", "barfoo"));
        assert!(!glob_match("foo", "fo"));
    }

    #[test]
    fn leading_star() {
        assert!(glob_match("*foo", "foo"));
        assert!(glob_match("*foo", "barfoo"));
        assert!(!glob_match("*foo", "foobar"));
    }

    #[test]
    fn trailing_star() {
        assert!(glob_match("foo*", "foo"));
        assert!(glob_match("foo*", "foobar"));
        assert!(!glob_match("foo*", "barfoo"));
    }

    #[test]
    fn double_star_collapses() {
        // Adjacent '*' is harmless — empty parts skip in the loop.
        assert!(glob_match("**foo", "foo"));
        assert!(glob_match("foo**bar", "foobar"));
    }

    #[test]
    fn middle_star() {
        assert!(glob_match("a*b", "ab"));
        assert!(glob_match("a*b", "axb"));
        assert!(!glob_match("a*b", "axc"));
    }

    #[test]
    fn trailing_literal_anchored_to_end_with_repeated_substring() {
        // The bug-fix case: trailing literal must rfind, not find.
        assert!(glob_match("*a*a", "aaaa"));
        assert!(glob_match("*a", "aaaa"));
        assert!(!glob_match("*a", "aaab"));
    }

    #[test]
    fn realistic_feat_glob() {
        assert!(glob_match("feat-*", "feat-login"));
        assert!(glob_match("feat-*", "feat-"));
        assert!(!glob_match("feat-*", "bug-feat-x"));
    }
}
