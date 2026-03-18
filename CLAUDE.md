# Claude Worktree RS - Project Guide for Claude Code

## Project Overview

Rust rewrite of [claude-worktree](https://github.com/DaveDev42/claude-worktree) (Python CLI, v0.10.54). Single static binary (~1.8MB), ~3ms startup.

## Project Structure

```
claude-worktree-rs/
├── Cargo.toml
├── src/
│   ├── main.rs                    # Entry point + command routing
│   ├── lib.rs                     # Module declarations
│   ├── cli.rs                     # clap derive CLI definitions
│   │   ├── completions.rs         # Tab completion (TODO)
│   │   └── global.rs              # Global mode filtering (TODO)
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
│       ├── path_cmd.rs            # _path internal command for cw-cd
│       ├── shell.rs               # Interactive shell in worktree
│       ├── stash.rs               # Worktree-aware stash save/list/apply
│       ├── worktree.rs            # create, delete, sync
│       └── launchers/
│           ├── foreground.rs      # Current terminal
│           ├── detached.rs        # setsid / DETACHED_PROCESS
│           ├── iterm.rs           # macOS AppleScript
│           ├── tmux.rs            # session/window/pane
│           ├── zellij.rs          # session/tab/pane
│           └── wezterm.rs         # window/tab/pane + readiness polling
├── .github/workflows/
│   ├── test.yml                   # CI: Test on 3 OS + lint
│   └── release.yml                # CD: Cross-compile + GitHub Release
├── README.md
├── CLAUDE.md                      # This file
└── LICENSE                        # BSD-3-Clause
```

## Technology Stack

- **Rust 1.75+**, fully synchronous (no async runtime)
- **clap** (derive): CLI framework
- **serde** + **serde_json**: Config/session/registry serialization
- **thiserror**: Error hierarchy
- **console**: Styled terminal output
- **dialoguer**: Interactive prompts
- **regex**: Branch name sanitization
- **dirs**: Platform config directories
- **pathdiff**: Relative path computation

## Development

```bash
# Build
cargo build

# Run
cargo run -- --help
cargo run -- list

# Test
cargo test

# Lint
cargo clippy
cargo fmt --check

# Release build
cargo build --release  # Output: target/release/cw (~1.8MB)
```

## Config Compatibility

Reads existing `~/.config/claude-worktree/config.json` from the Python version.
Same git config metadata keys (`branch.*.worktreeBase`, `worktree.*.basePath`, `worktree.*.intendedBranch`).
Same session storage paths (`~/.config/claude-worktree/sessions/`).

## Code Conventions

- Type hints via Rust's type system (no dynamic typing)
- Error handling: `Result<T>` with `CwError` enum (7 variants)
- No `unwrap()` in production paths — use `?` or handle gracefully
- Output via `println!` with `console::style()` for colors
- Git operations via `std::process::Command`, not libgit2
- Zero clippy warnings policy
