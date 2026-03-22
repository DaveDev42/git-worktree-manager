# git-worktree-manager (gw) - Project Guide for Claude Code

## Project Overview

**git-worktree-manager** (`gw`) is a CLI tool integrating git worktree with AI coding assistants. Successor to [claude-worktree](https://github.com/DaveDev42/claude-worktree) (Python), rewritten in Rust. Single static binary (~1.9MB), ~3ms startup.

## Project Structure

```
git-worktree-manager/
├── Cargo.toml                     # Package: git-worktree-manager, bin: gw
├── src/
│   ├── main.rs                    # Entry point + command routing
│   ├── lib.rs                     # Module declarations
│   ├── cli.rs                     # clap derive CLI definitions
│   ├── config.rs                  # serde-based typed config
│   ├── constants.rs               # LaunchMethod enum, presets, sanitization
│   ├── console.rs                 # Styled output helpers (console crate)
│   ├── error.rs                   # thiserror error hierarchy
│   ├── git.rs                     # Git command wrapper
│   ├── hooks.rs                   # Hook execution + CRUD
│   ├── registry.rs                # Global repository registry
│   ├── session.rs                 # AI session metadata
│   ├── shared_files.rs            # .cwshare file copying
│   ├── shell_functions.rs         # Shell function generation (bash/zsh/fish)
│   ├── update.rs                  # Auto-update via GitHub Releases
│   └── operations/
│       ├── ai_tools.rs            # AI tool launcher dispatch
│       ├── backup.rs              # git bundle backup/restore
│       ├── clean.rs               # Batch worktree cleanup
│       ├── config_ops.rs          # change-base, export, import
│       ├── diagnostics.rs         # doctor health check
│       ├── display.rs             # list, status, tree, stats, diff
│       ├── git_ops.rs             # PR creation, merge workflow
│       ├── helpers.rs             # resolve_worktree_target, metadata
│       ├── path_cmd.rs            # _path internal command for gw-cd
│       ├── shell.rs               # Interactive shell in worktree
│       ├── stash.rs               # Worktree-aware stash save/list/apply
│       ├── worktree.rs            # create, delete, sync
│       └── launchers/             # 6 terminal launchers (18 variants)
├── tests/                         # 66 integration + unit tests
├── .github/workflows/             # CI (test.yml) + CD (release.yml)
├── README.md
├── CLAUDE.md                      # This file
└── LICENSE                        # BSD-3-Clause
```

## Development

```bash
cargo build                        # Build
cargo run -- --help                # Run
cargo test                         # Test (460 tests (11 ignored))
cargo clippy                      # Lint
cargo fmt --check                  # Format check
cargo build --release              # Release: target/release/gw (~1.9MB)
```

## Config Compatibility

Reads existing `~/.config/claude-worktree/config.json` from the Python version.
Same git config metadata keys and session storage paths.

## Code Conventions

- Error handling: `Result<T>` with `CwError` enum, no `unwrap()` in production
- Output: `println!` with `console::style()` for colors
- Git operations: `std::process::Command`, not libgit2
- Zero clippy warnings policy
- Fully synchronous (no async runtime)
