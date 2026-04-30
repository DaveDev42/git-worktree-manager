/// Operations module — business logic for all commands.
pub mod ai_tools;
pub mod backup;
pub mod busy;
pub mod busy_messages;
pub mod claude_process;
pub mod claude_session;
pub mod clean;
pub mod config_ops;
pub mod delete_batch;
pub mod diagnostics;
pub mod display;
pub mod global_ops;
pub mod guard;
pub mod helpers;
pub mod launchers;
pub mod lockfile;
pub mod path_cmd;
pub mod pr_cache;
pub mod setup_claude;
pub mod shell;
pub mod spawn_spec;
pub mod stash;
#[cfg(test)]
pub(crate) mod test_env;
pub mod worktree;
