//! `gw exec <target> <cmd>` — single-worktree command dispatch.

use std::io::Write;
use std::path::Path;

use crate::error::Result;
use crate::git;
use crate::operations::helpers::resolve_target_strict;

pub fn exec_in_target<W: Write>(
    cwd: &Path,
    target: &str,
    cmd: &[String],
    out: &mut W,
) -> Result<i32> {
    let repo_root = git::get_main_repo_root(Some(cwd))?;
    let resolved = resolve_target_strict(&repo_root, target)?;
    let (s, code) = super::run::run_capture_one(&resolved.name, &resolved.path, cmd)?;
    let _ = out.write_all(s.as_bytes());
    Ok(code)
}
