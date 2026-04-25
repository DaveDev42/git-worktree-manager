//! Plugin manifest content. Static blob — versioned with the binary.

pub fn content() -> &'static str {
    "{\n  \"name\": \"gw\",\n  \"version\": \"1\",\n  \"description\": \"git-worktree-manager plugin: delegate tasks to worktrees and manage multi-worktree workflows safely.\",\n  \"author\": \"git-worktree-manager\"\n}\n"
}
