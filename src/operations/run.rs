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

    let _ = jobs; // sequential for now; parallel handled in Phase 5.4

    let mut last_failure: i32 = 0;
    for w in targets {
        let exit = spawn_one(w, cmd, out)?;
        if exit != 0 {
            last_failure = exit;
            if !continue_on_error {
                break;
            }
        }
    }
    Ok(last_failure)
}

fn spawn_one<W: Write>(w: &scope::ScopedWorktree, cmd: &[String], out: &mut W) -> Result<i32> {
    let prefix = format!("[{}] ", w.name);
    let mut child = Command::new(&cmd[0])
        .args(&cmd[1..])
        .current_dir(&w.path)
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
    let _ = out.write_all(so.as_bytes());
    let _ = out.write_all(se.as_bytes());

    let status = child.wait()?;
    Ok(status.code().unwrap_or(1))
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
