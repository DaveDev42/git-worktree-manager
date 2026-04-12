# Worktree Busy Detection — Design

## Problem

현재 `gw`는 worktree가 실제로 사용 중인지 제대로 판단하지 못한다.
`get_worktree_status`는 "현재 shell의 cwd가 worktree 안에 있는가"만
검사하기 때문에, 다른 터미널/Claude Code 세션이 해당 worktree에서
작업 중임에도 `gw delete`나 `gw clean`이 이를 감지하지 못하고
삭제해버린다.

겪는 시나리오:

1. 다른 터미널 탭에서 `claude`가 실행 중인데 `gw clean --merged`가
   해당 worktree를 제거
2. `gw shell`/`gw start`로 연 세션이 살아있는데 다른 세션에서
   `gw delete`가 제거

## Goal

worktree가 "작업 중"인지 신뢰 가능하게 판정하고, 삭제 경로에서
이를 존중한다. 동시에 Claude Code가 자기 worktree를 정상적으로
정리하는 흐름은 막지 않는다.

## Design

### 1. Busy 감지 — 하이브리드

두 가지 신호를 모두 사용하고, **하나라도 걸리면 busy**로 판정한다.

**신호 A — Lockfile (명시적)**

- 위치: `.git/worktrees/<name>/gw-session.lock`
- 내용 (JSON): `{ "pid": u32, "started_at": i64, "cmd": "claude" | "shell" | ... }`
- 생성 시점: `gw shell`, `gw start`, AI 런처(`ai_tools.rs`)가
  worktree 안에서 프로세스를 띄울 때
- 제거 시점: 해당 프로세스가 정상 종료될 때 (RAII guard)
- 읽을 때 PID 살아있는지 `kill(pid, 0)`로 확인.
  죽어있으면 stale로 간주하고 자동 제거한다.

**신호 B — 프로세스 스캔 (암묵적)**

worktree 경로를 cwd로 가진 프로세스를 탐지한다. `gw` 외부에서
`cd`로 들어가 claude/vim/shell을 띄운 경우를 잡는다.

- macOS: `lsof -a -d cwd -F pn +D <path>`
- Linux: `/proc/*/cwd` symlink를 읽어 경로 비교
- 기타 플랫폼: 비활성화 (lockfile만 사용) — graceful degrade
- 명령 실패/권한 부족 시에도 gracefully degrade

**자기 자신 제외**

현재 프로세스의 PID와 조상 체인(`getppid` 재귀)을 수집해서 busy
목록에서 제외한다. Claude Code가 `gw delete`를 호출할 때 자기
자신 혹은 자기를 띄운 터미널 프로세스가 busy로 잡혀서 오탐하는
것을 막는다.

`GW_SESSION_ID` 같은 환경변수는 도입하지 않는다 (YAGNI).

### 2. 삭제 명령 동작

**`gw delete <branch>`**

- busy 아님: 기존 동작 그대로
- busy + TTY: busy 프로세스 목록(PID, cmd, cwd)을 출력하고
  `"이 worktree를 삭제할까요? (y/N)"` 프롬프트. 기본값 No.
- busy + non-TTY (파이프/CI/Claude Code에서 호출): busy 목록을
  stderr에 출력하고 에러 종료 (exit code ≠ 0).
  Claude Code는 에러를 보고 판단할 수 있다.
- `--force`: busy 여부와 관계없이 경고만 찍고 진행

**`gw clean`**

- busy worktree는 **자동 스킵** (배치 명령이라 프롬프트 부적합)
- 요약에 `skipped: N (busy)` 추가
- 각 worktree 행 옆에 `(busy: PID 12345 claude)` 표기
- `--force`: busy 무시하고 삭제
- `--dry-run`: busy 표기만 (어차피 삭제 안 함)

### 3. Status 값 확장

`get_worktree_status`에 `"busy"` 추가.

우선순위: `stale > busy > active(current) > merged > pr-open > modified > clean`

`busy`가 `active`보다 우선인 이유: 현재 shell이 cwd를 걸고 있는
경우라도 다른 프로세스가 더 의미있는 점유 상태이기 때문.
단, 자기 자신 제외 규칙이 적용되므로 실제로는 "나 말고 다른
점유자가 있는" 경우에만 busy로 뜬다.

`gw list`, `gw status`, `gw tree`의 표시에 반영한다.

### 4. 파일 구조

**신규**

- `src/operations/busy.rs`
  - `pub struct BusyInfo { pid: u32, cmd: String, cwd: PathBuf, source: BusySource }`
  - `pub enum BusySource { Lockfile, ProcessScan }`
  - `pub fn detect_busy(worktree_path: &Path) -> Vec<BusyInfo>`
  - `fn self_process_tree() -> HashSet<u32>`
  - `fn scan_processes_by_cwd(path: &Path) -> Vec<BusyInfo>` (플랫폼 분기)

- `src/operations/lockfile.rs`
  - `pub struct SessionLock { path: PathBuf }` — RAII drop으로 제거
  - `pub fn acquire(worktree: &Path, cmd: &str) -> Result<SessionLock>`
  - `pub fn read(worktree: &Path) -> Option<LockEntry>` — stale PID 자동 정리
  - `pub struct LockEntry { pid: u32, started_at: i64, cmd: String }`

**수정**

- `src/operations/display.rs`: `get_worktree_status`에 busy 분기
  추가. 우선순위 갱신. 테스트 추가.
- `src/operations/worktree.rs`: `delete_worktree`에서 busy 체크 +
  TTY 분기 (`atty` 또는 `std::io::IsTerminal`).
- `src/operations/clean.rs`: busy 스킵 로직 + 요약 포맷.
- `src/operations/shell.rs`: shell 진입 시 `SessionLock::acquire`.
- `src/operations/ai_tools.rs` / `launchers/*`: AI 런처 진입 시
  `SessionLock::acquire`. RAII로 런처 종료 시 자동 해제.
- `src/lib.rs`: 새 모듈 선언.

### 5. 에러 처리

- Lockfile 쓰기 실패: 경고 로그만 찍고 진행 (세션 시작 자체는
  막지 않음). 디스크/권한 문제로 세션을 시작 못 하면 UX가 나쁨.
- 프로세스 스캔 실패: silently skip, lockfile 신호만 사용.
- 삭제 시 busy 감지 실패(예: lsof 부재): lockfile만으로 판정.
  false negative가 날 수 있지만 현재보다 나쁘진 않음.

### 6. 테스트

- **Lockfile 단위 테스트**
  - acquire → read → drop → 파일 없어짐
  - stale PID가 있는 lockfile 읽으면 None + 파일 제거
  - 동일 worktree에 두 번 acquire 시도 (동시성)
- **프로세스 스캔 통합 테스트** (macOS/Linux only, ignored 기본)
  - 임시 프로세스를 worktree cwd로 spawn → 탐지 검증
  - 프로세스 kill 후 재스캔 → 미탐지 확인
- **자기 제외 테스트**
  - 테스트 프로세스 자신이 busy 목록에 없는지
  - 부모 프로세스(test runner)도 제외되는지
- **삭제 경로 테스트**
  - busy + TTY: 프롬프트 트리거 (TTY mock)
  - busy + non-TTY: 에러 종료
  - `--force`: busy여도 진행
  - `gw clean` busy 스킵 + 요약 카운트

### 7. Non-Goals

- Windows 지원 (기존에도 제한적). Linux/macOS만 1차 대상.
- 네트워크 drive 상의 worktree의 cross-host 감지.
- 프로세스의 "유휴" vs "활동" 구분. 점유하고 있으면 busy.
- Lockfile 타임아웃/만료. PID liveness 검사로 충분.

## Open Questions

없음. 진행 가능.
