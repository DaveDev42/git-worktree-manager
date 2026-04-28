# Release & PR procedure

릴리스를 만들거나 release-please PR을 merge하기 전에 read.
일상 작업에서는 CLAUDE.md의 "Git & Release" 섹션 핵심 룰만으로 충분.

## Branch Protection (`main`)

`main`은 다음 규칙으로 보호됨 — git 작업 스크립트 작성 시 인지:

- Required status check: **`ci-gate`** must pass (and branch must be up-to-date with `main` — `strict: true`).
- **Linear history required** — only squash or rebase merges; merge commits will be rejected.
- Force push and branch deletion are disabled.
- Unresolved PR conversations block merging.
- Admin enforcement is **off** so release-please automation keeps working; do not push directly to `main` regardless.

## Commit & Release Convention

- **Default to patch version bumps.** Unless the user explicitly asks for a major or minor bump, every change (including API-breaking ones in 0.x) must ship as a patch release.
- **Never use `feat!`, `fix!`, or a `BREAKING CHANGE:` footer** in PR titles, merge-commit messages, or commit messages. These escalate release-please to major bumps automatically (e.g. 0.x → 1.0.0). Use plain `feat:` / `fix:` / `refactor:` / `chore:` instead, and describe breaking changes in the PR body and migration notes rather than the commit prefix.
- **Manual major/minor bump**: when a major/minor release is explicitly requested, push a commit to `main` with a `Release-As: x.y.z` footer, or temporarily set `release-as` in `release-please-config.json` via a chore PR, then remove it in a follow-up chore PR after the release ships.
- Since this repo squash-merges, **only the PR title** lands on `main` as the conventional commit that release-please reads. Always write the PR title as a valid conventional commit (`type: subject`) — branch commits are discarded on merge and do not feed release-please.
- The same banned-prefix rule applies to **PR titles**: never start a PR title with `feat!`, `fix!`, or include `BREAKING CHANGE:` in the squash subject/body.

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
