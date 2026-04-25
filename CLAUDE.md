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

## Claude Code Integration

Run `gw setup-claude` to install the Claude Code skill for this project.
Once installed, use the `/gw` slash command or natural language to delegate coding tasks to isolated worktrees.
Each delegated task runs in its own branch with a separate Claude Code instance.

## Config Compatibility

Reads existing `~/.config/claude-worktree/config.json` from the Python version.
Same git config metadata keys and session storage paths.

## Git & Release

- PR merge method: **squash merge** (`--squash`). Merge commits and rebase merges are disabled at the repo level.
- The squash commit uses **PR title as commit subject** and **PR body as commit body** (GitHub repo setting).
- Release process: [release-please](https://github.com/googleapis/release-please) automates versioning via conventional commits
- Commit messages: conventional commits (`feat:`, `fix:`, `perf:`, `chore:`, etc.)

### Branch Protection (`main`)

`main` is protected with the following rules — keep these in mind when scripting git operations:

- Required status check: **`ci-gate`** must pass (and branch must be up-to-date with `main` — `strict: true`).
- **Linear history required** — only squash or rebase merges; merge commits will be rejected.
- Force push and branch deletion are disabled.
- Unresolved PR conversations block merging.
- Admin enforcement is **off** so release-please automation keeps working; do not push directly to `main` regardless.

### Commit & Release Convention

- **Default to patch version bumps.** Unless the user explicitly asks for a major or minor bump, every change (including API-breaking ones in 0.x) must ship as a patch release.
- **Never use `feat!`, `fix!`, or a `BREAKING CHANGE:` footer** in PR titles, merge-commit messages, or commit messages. These escalate release-please to major bumps automatically (e.g. 0.x → 1.0.0). Use plain `feat:` / `fix:` / `refactor:` / `chore:` instead, and describe breaking changes in the PR body and migration notes rather than the commit prefix.
- **Manual major/minor bump**: when a major/minor release is explicitly requested, push a commit to `main` with a `Release-As: x.y.z` footer, or temporarily set `release-as` in `release-please-config.json` via a chore PR, then remove it in a follow-up chore PR after the release ships.
- Since this repo squash-merges, **only the PR title** lands on `main` as the conventional commit that release-please reads. Always write the PR title as a valid conventional commit (`type: subject`) — branch commits are discarded on merge and do not feed release-please.
- The same banned-prefix rule applies to **PR titles**: never start a PR title with `feat!`, `fix!`, or include `BREAKING CHANGE:` in the squash subject/body.

## Code Conventions

- Error handling: `Result<T>` with `CwError` enum, no `unwrap()` in production
- Output: `println!` with `console::style()` for colors
- Git operations: `std::process::Command`, not libgit2
- Zero clippy warnings policy
- Fully synchronous (no async runtime)

## Pre-Release Checklist

**중요: 아래 명령들은 옵션 그대로 사용. `--all-targets --all-features` 조합 누락 시 drift가 검출되지 않는다.**

릴리스를 만드는 커밋 (또는 `release-please` 자동 생성 PR을 merge하기 전) 직전에 확인:

```bash
cargo clippy --all-targets --all-features -- -D warnings   # 0 warnings 강제
cargo test --all-targets                                    # 전체 target (bin/lib/tests/examples)
cargo fmt --check                                           # format drift 없음
cargo build --release                                       # target/release/gw 생성
ls -l target/release/gw                                     # binary size regression 확인 (~1.9MB 기준)
```

이 체크리스트는 CI가 대부분 커버하지만, **clippy는 `--all-targets --all-features` 조합일 때만** 다음 정도 drift가 잡힌다:
- `#[cfg(test)]` 블록 안의 lint
- feature-gated 코드 (`--all-features`가 없으면 skip)
- test/examples/benches target의 warning

cross-platform smoke test가 필요한 변경 (new syscall, path 처리, external process invocation) 은 CI `test` job의 Linux/macOS/Windows matrix를 반드시 통과시킨 뒤 release 한다. Windows-specific 실패는 로컬에서 재현하기 어려우므로 CI 결과에 의존할 것.
