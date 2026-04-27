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
