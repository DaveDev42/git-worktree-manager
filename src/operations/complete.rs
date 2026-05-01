//! Internal helper for shell tab completion: prints valid positional
//! target tokens (worktree names + branch names) one per line.
//!
//! Consumed by the bash/zsh/fish/powershell completion scripts in
//! shell_functions.rs to populate dynamic completions for `gw rm`,
//! `gw resume`, `gw spawn`, and `gw exec`.

use std::collections::BTreeSet;

use crate::error::Result;

pub fn print_completion_targets() -> Result<()> {
    // cwd-scoped: only the repo we're inside (or the current scope, by
    // discover_scope semantics). Errors are silenced — the shell doesn't
    // care WHY there's nothing to complete; it just gets an empty list.
    let cwd = match std::env::current_dir() {
        Ok(p) => p,
        Err(_) => return Ok(()),
    };
    let scope = match crate::scope::discover_scope(&cwd) {
        Ok(s) => s,
        Err(_) => return Ok(()),
    };

    let mut tokens: BTreeSet<String> = BTreeSet::new();
    for w in scope.worktrees() {
        // worktree name (file_name of the worktree path)
        if let Some(name) = w.path.file_name().and_then(|n| n.to_str()) {
            if !name.is_empty() {
                tokens.insert(name.to_string());
            }
        }
        // branch name (skip detached)
        if let Some(branch) = &w.branch {
            tokens.insert(branch.clone());
        }
    }

    for tok in tokens {
        println!("{}", tok);
    }
    Ok(())
}
