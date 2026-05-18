# git-worktree-manager (gw) - Project Guide for Claude Code

## Project Overview

**git-worktree-manager** (`gw`) is a CLI tool integrating git worktree with AI coding assistants. Successor to [claude-worktree](https://github.com/DaveDev42/claude-worktree) (Python), rewritten in Rust. Single static binary (~1.9MB), ~3ms startup.

## Project Structure

```
git-worktree-manager/
├── Cargo.toml                     # Package: git-worktree-manager, bin: gw
├── src/
│   ├── bin/gw.rs                  # `gw` CLI entry (8 MiB stack, calls entrypoint::run)
│   ├── lib.rs                     # Module declarations
│   ├── entrypoint.rs              # Single dispatch entry (run function)
│   ├── cli.rs                     # clap derive CLI definitions
│   ├── cli/completions.rs         # Shell completion generation
│   ├── config.rs                  # serde-based typed config (global)
│   ├── repo_config.rs             # Per-repo `.cwconfig.json`
│   ├── scope.rs                   # Global vs repo config scope resolution
│   ├── constants.rs               # LaunchMethod enum, presets, sanitization
│   ├── console.rs                 # Styled output helpers (console crate)
│   ├── cwshare_setup.rs           # .cwshare bootstrap
│   ├── error.rs                   # thiserror error hierarchy
│   ├── git.rs                     # Git command wrapper
│   ├── hooks.rs                   # Hook execution + CRUD
│   ├── messages.rs                # Output message templates
│   ├── prompt_source.rs           # AI prompt assembly (resolve_prompt)
│   ├── session.rs                 # AI session metadata
│   ├── shared_files.rs            # .cwshare file copying
│   ├── shell_functions.rs         # Shell function generation (bash/zsh/fish)
│   ├── tui/                       # Ratatui-based pickers (arrow/multi-select, list view)
│   ├── update.rs                  # Auto-update via GitHub Releases
│   └── operations/
│       ├── ai_tools.rs            # AI tool launcher dispatch
│       ├── busy.rs                # Worktree busy-state detection
│       ├── busy_messages.rs       # Busy-state user messaging
│       ├── claude_process.rs      # Claude CLI process detection
│       ├── claude_session.rs      # Claude session file inspection
│       ├── claude_settings.rs     # Claude settings.json read/write
│       ├── complete.rs            # `gw complete` dynamic completion source
│       ├── diagnostics.rs         # doctor health check
│       ├── display.rs             # list, status, tree, stats, diff
│       ├── exec.rs                # `gw exec` arbitrary command runner
│       ├── guard.rs               # PreToolUse(Bash) hook injection
│       ├── helpers.rs             # resolve_worktree_target, metadata
│       ├── lockfile.rs            # Cross-process worktree locks
│       ├── path_cmd.rs            # _path internal command for gw-cd
│       ├── pr_cache.rs            # GitHub PR metadata cache
│       ├── rm_batch.rs            # Batch worktree removal
│       ├── run.rs                 # `gw run` shell-in-worktree
│       ├── setup_claude/          # `gw setup-claude` skill installer
│       ├── spawn_spec.rs          # SpawnSpec + persistent gw-spawn-last.json
│       ├── test_env.rs            # Test-only env helpers
│       ├── worktree.rs            # create, delete, sync
│       └── launchers/             # 6 terminal launchers (foreground/detached/iterm/wezterm/tmux/zellij)
├── tests/                         # Integration + unit tests (~460, 11 ignored)
├── .github/workflows/             # test.yml (CI) + release-please.yml (release-please + build + GH release + tap update + crates.io publish)
├── .claude/commands/release.md    # `/release` slash command for patch releases
├── README.md
├── CLAUDE.md                      # This file
└── LICENSE                        # BSD-3-Clause
```

## Development

```bash
cargo build                                            # Build
cargo run -- --help                                    # Run
cargo test                                             # Test (~460, 11 ignored)
cargo clippy --all-targets --all-features -- -D warnings  # Lint (zero-warning)
cargo fmt --check                                      # Format check
cargo build --release                                  # Release: target/release/gw (~1.9MB)
```

CI runs default `cargo clippy`; the `--all-targets --all-features` form catches `#[cfg(test)]` and feature-gated drift that the release process cares about. Run it locally before tagging a release.

## Claude Code Integration

Run `gw setup-claude` to install the Claude Code integration for this project.
Once installed, ask Claude in natural language to delegate tasks; gw will route
Claude's `Agent(isolation: "worktree")` calls through the registered hooks.
Each delegated task runs in its own branch with a separate Claude Code instance.

릴리스 작업: 패치 릴리스("release new patch version" 등)는 `/release` 슬래시 명령으로 진행 (`.claude/commands/release.md`). ad-hoc git/cargo 명령으로 매번 재구성 X.

## Config Compatibility

Reads existing `~/.config/claude-worktree/config.json` from the Python version.
Same git config metadata keys and session storage paths.

## Git & Release

- `main` is branch-protected: `ci-gate` required, linear history, no force pushes.
- PR merge method: **squash merge** (`--squash`). Merge commits and rebase merges are disabled at the repo level.
- The squash commit uses **PR title as commit subject** and **PR body as commit body** (GitHub repo setting).
- Release process: [release-please](https://github.com/googleapis/release-please) automates versioning via conventional commits.
- The `release-please.yml` workflow handles the full release pipeline in one file: release-please PR → multi-platform build → GitHub Release publish → Homebrew tap update (DaveDev42/homebrew-tap) → crates.io publish.
- Commit messages: conventional commits (`feat:`, `fix:`, `perf:`, `chore:`, etc.).
- **PR title은 valid conventional commit으로 작성** (`type: subject`). squash merge라 PR title만 release-please가 읽음.
- **`feat!` / `fix!` / `BREAKING CHANGE:` 절대 금지** — major bump 자동 트리거 (0.x → 1.0.0). breaking change는 PR body에 설명.

릴리스 절차 — 사람이 직접 돌릴 일은 거의 없음. release-please가 PR을 만들고, `/release` 명령이 gate/merge/watch/verify(tap 동기화 포함)까지 자동화한다 (`.claude/commands/release.md` 참조). Manual major/minor bump가 필요하면 `Release-As: x.y.z` footer 커밋을 `main`에 push (release-please가 자동 인식).

## Code Conventions

- Error handling: `Result<T>` with `CwError` enum, no `unwrap()` in production
- Output: `println!` with `console::style()` for colors
- Git operations: `std::process::Command`, not libgit2
- Zero clippy warnings policy
- Fully synchronous (no async runtime)
- Subagent 호출 시 항상 `model` 명시 — 생략하면 부모 모델(Opus) 상속 → 단순 작업도 Opus 비용.
  - 코드베이스 탐색 (3+ grep/find/read 묶음): `Explore` agent + `model=haiku`
  - 단순 lookup / 짧은 요약: `general-purpose` + `model=haiku`
  - 코드 구현·디버깅·리팩터·테스트 작성: `general-purpose` + `model=sonnet`
  - 코드 review: `general-purpose` + `model=sonnet`
  - PR 마무리: `pr-shipper` (정의에 model 잡혀있음)
  - 어려운 설계 추론: `Plan` 또는 `general-purpose` + `model=opus`

