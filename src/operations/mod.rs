/// Operations module — business logic for all commands.
pub mod ai_tools;
pub mod busy;
pub mod busy_messages;
pub mod claude_process;
pub mod claude_session;
pub mod diagnostics;
pub mod display;
pub mod exec;
pub mod guard;
pub mod helpers;
pub mod launchers;
pub mod lockfile;
pub mod path_cmd;
pub mod pr_cache;
pub mod rm_batch;
pub mod run;
pub mod setup_claude;
pub mod spawn_spec;
#[cfg(test)]
pub(crate) mod test_env;
pub mod worktree;
