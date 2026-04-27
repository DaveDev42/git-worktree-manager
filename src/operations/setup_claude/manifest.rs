//! JSON manifests for the local Claude Code marketplace + plugin.
//!
//! `marketplace.json` describes one plugin, `gw`, sourced from the sibling
//! `./gw-plugin` directory. `plugin.json` carries the binary's Cargo
//! version so re-running `gw setup-claude` after upgrading triggers a real
//! `claude plugin update` (versions must differ for cache to refresh).

const PLUGIN_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn marketplace_json() -> &'static str {
    // Static blob — version-independent.
    concat!(
        "{\n",
        "  \"name\": \"gw-local\",\n",
        "  \"owner\": { \"name\": \"git-worktree-manager\" },\n",
        "  \"plugins\": [\n",
        "    {\n",
        "      \"name\": \"gw\",\n",
        "      \"source\": \"./gw-plugin\",\n",
        "      \"description\": \"git-worktree-manager: delegate tasks to worktrees and manage multi-worktree workflows safely.\"\n",
        "    }\n",
        "  ]\n",
        "}\n"
    )
}

pub fn plugin_json() -> String {
    format!(
        "{{\n  \"name\": \"gw\",\n  \"version\": \"{}\",\n  \"description\": \"git-worktree-manager plugin: /gw delegate + manage skill.\",\n  \"author\": {{ \"name\": \"git-worktree-manager\" }}\n}}\n",
        PLUGIN_VERSION
    )
}

/// Temporary shim kept until Task 7 rewrites `mod.rs`. Returns the OLD
/// (broken) `plugin.json` body so the existing call site in `mod.rs`
/// continues to compile and the test_setup_claude_plugin.rs integration
/// test (which Task 7 deletes) keeps passing in the meantime.
#[deprecated(note = "removed in Task 7")]
pub fn content() -> &'static str {
    "{\n  \"name\": \"gw\",\n  \"version\": \"1\",\n  \"description\": \"git-worktree-manager plugin: delegate tasks to worktrees and manage multi-worktree workflows safely.\",\n  \"author\": \"git-worktree-manager\"\n}\n"
}
