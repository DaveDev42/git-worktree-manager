---
description: Cut a patch release of git-worktree-manager (release-please + brew tap verified)
---

Drive a patch release end-to-end. The release pipeline is automated by
release-please + `.github/workflows/release-please.yml` (multi-platform
build, GitHub Release publish, Homebrew tap update, crates.io publish).
Your job is to gate, merge, watch, and **independently verify the tap
actually got the new version** — even when the workflow reports green,
the only durable signal that downstream users will see the new release
is a real `brew upgrade` resolving to the new version.

## Inputs

None. Always operates on `main`, always patch-bump (release-please
decides the version). Major/minor bumps are out of scope — push a
`Release-As: x.y.z` footer commit to `main` (release-please picks it
up automatically) instead of running this command.

`main` is branch-protected: `ci-gate` required, linear history, no
force pushes, admin enforcement off so release-please automation keeps
working. Never use `feat!` / `fix!` / `BREAKING CHANGE:` — they escalate
release-please to a major bump (0.x → 1.0.0) automatically; describe
breaking changes in the PR body instead.

## Procedure

Stop and report on any failure. Do **not** auto-recover — except for the
single bounded retrigger in Step 2 that covers the documented
`GITHUB_TOKEN` workflow-trigger gap.

### Step 0 — Preconditions

```sh
git rev-parse --abbrev-ref HEAD          # must be: main
git status --porcelain                    # must be: empty
git fetch origin && git status            # must be: up to date with origin/main
gh auth status                            # must succeed
```

Abort if any check fails. Report exactly which one.

### Step 1 — Locate or trigger the release-please PR

```sh
gh pr list --label "autorelease: pending" --state open \
  --json number,title,headRefName,headRefOid
```

- **PR found** → record `number`, `title`, head SHA. Continue.
- **No PR** → trigger the workflow and poll:

  ```sh
  gh workflow run release-please.yml --ref main
  ```

  Then every 30 seconds, re-run the `gh pr list` query above. Stop after
  5 minutes (10 polls). If still no PR, abort with:

  > "release-please did not produce a PR within 5 minutes. Likely no
  > releasable conventional commits since the last tag. Check
  > `git log v$(git describe --tags --abbrev=0)..main --oneline` and
  > confirm there are `feat:` / `fix:` / `perf:` commits."

The PR title format is `chore(main): release X.Y.Z`. Extract `X.Y.Z` —
this is the **target version**. All later steps reference it as
`<VERSION>` and the tag as `v<VERSION>`.

### Step 2 — CI gate + local clippy drift check

Confirm the PR's `ci-gate` status check is green:

```sh
gh pr view <num> --json statusCheckRollup \
  --jq '.statusCheckRollup[] | select(.name=="ci-gate") | .conclusion'
```

- `SUCCESS` → continue.
- `null` / `IN_PROGRESS` / `PENDING` → poll every 30 seconds, max 10
  minutes. Then abort if still not green.
- `FAILURE` / `CANCELLED` → abort immediately, surface the failing check
  URL via `gh pr checks <num>`.

**Empty rollup (no `ci-gate` row at all)** — the entire
`statusCheckRollup` array is `[]` and the polling loop above will
never observe anything. Distinguish this from "in progress" before
the 10-minute timeout. To avoid racing GitHub's run-queue latency,
wait at least **60 seconds** since the PR's current head was pushed
before treating empty rollup as a trigger gap (two 30s poll cycles).
Then check whether `Test` ran against the PR's current head — use
`headRefName` from the Step 1 `gh pr list` output as `<pr-branch>`:

```sh
PR_HEAD=$(gh pr view <num> --json headRefOid --jq '.headRefOid')
gh run list --branch <pr-branch> --workflow=Test --limit=5 \
  --json headSha,conclusion --jq ".[] | select(.headSha==\"$PR_HEAD\")"
```

- **Query returns one or more rows** → `Test` is running (or already
  ran) against the current head. The gap does not apply; do **not**
  retrigger. Fall back into the normal `ci-gate` polling loop above.
  This is also the re-run case: if a prior invocation already pushed
  `chore: retrigger ci-gate`, that earlier push fired `Test` and this
  query now sees it.
- **Query returns nothing** → the workflow was not triggered for the
  current head. Almost always because release-please bot used
  `GITHUB_TOKEN` to force-update the PR branch (GitHub's documented
  policy: pushes made with `GITHUB_TOKEN` do not trigger workflows
  that would otherwise be triggered by `push` or `pull_request`).
  Proceed with the retrigger recipe below.

This is the **only** auto-recovery path this skill takes. The recipe
is bounded, idempotent, and side-effect-free:

```sh
# Clear any stale temp branch from a prior aborted run, then recreate
# from the current PR head, empty commit, push back.
git branch -D rp-retrigger 2>/dev/null || true
git fetch origin <pr-branch>
git checkout -b rp-retrigger origin/<pr-branch>
git commit --allow-empty -m "chore: retrigger ci-gate"
git push origin HEAD:<pr-branch>
git checkout main && git branch -D rp-retrigger
```

Why this is safe to do automatically here (and only here):

- The push originates from your user credentials, so the `Test`
  workflow does fire on the new head.
- release-please's squash-merge keeps the PR **title** as the commit
  subject (`chore(main): release X.Y.Z`), so the empty commit's
  subject is discarded at merge time. The `release-please.yml` job
  on `main` only needs that PR-title subject to fire; it doesn't read
  the squashed-away commits.
- No new code, no new artifact — purely a workflow-trigger nudge.

After pushing, fall back into the normal `ci-gate` polling loop
above with a **fresh 10-minute window** starting at the retrigger
push (not carried over from Step 2's start). If `ci-gate` still does
not appear within that window, abort: something other than the
`GITHUB_TOKEN` trigger gap is in play, and the skill should not keep
nudging.

Then run the drift check that CI does not cover:

```sh
cargo clippy --all-targets --all-features -- -D warnings
```

The `--all-targets --all-features` combination catches drift the default
CI clippy run misses: `#[cfg(test)]` lints, feature-gated code, and
test/example/bench target warnings. Without both flags, those paths
are skipped. `cargo test`, `cargo fmt --check`, and `cargo build
--release` are CI-covered — do **not** re-run them locally.

If clippy reports anything, abort. Do not "fix and retry" inline; the
release should not paper over a clippy violation.

### Step 3 — Merge

```sh
gh pr merge <num> --squash --auto
```

`--auto` is safe: `ci-gate` already passed in Step 2. Poll until merged:

```sh
gh pr view <num> --json state --jq '.state'   # poll until: MERGED
```

Capture the resulting commit SHA on `main`:

```sh
git fetch origin main
MERGE_SHA=$(git rev-parse origin/main)
```

The squash subject is the PR title (`chore(main): release X.Y.Z`),
which is the conventional commit release-please needs to fire next.

### Step 4 — Watch the `release-please.yml` run

Find the run triggered by the merge and watch it:

```sh
RUN_ID=$(gh run list --workflow=release-please.yml --branch=main --limit=1 \
  --json databaseId,headSha \
  --jq ".[] | select(.headSha==\"$MERGE_SHA\") | .databaseId")

# If RUN_ID is empty, the run has not appeared yet — poll every 15s for
# up to 2 minutes.

gh run watch "$RUN_ID" --exit-status
```

- `--exit-status` fails the watch on any job failure. On non-zero exit:

  ```sh
  gh run view "$RUN_ID" --log-failed
  ```

  Show the output to the user, name the failed step, abort.

- On success the run is trusted (the tap step now uses
  `DaveDev42/homebrew-tap-release@v1`, which fails the job rather than
  emitting a warning if the tap push doesn't land). Step 5 still
  cross-checks the tap repo to catch any drift between the workflow's
  view of success and what the tap repo actually contains.

### Step 5 — Tap repo sanity check

```sh
gh api repos/DaveDev42/homebrew-tap/commits/main \
  --jq '.commit.message'
```

Expected: a message containing the literal `git-worktree-manager <VERSION>`
(matching the `<VERSION>` from Step 1). The current commit-message format
emitted by `homebrew-tap-release@v1` is `git-worktree-manager <VERSION>`,
but match by version-substring rather than by exact prefix so this step
keeps working if the action upgrades its message format. If the version
string is absent, abort with the actual message — the workflow reported
success but the tap is out of sync.

Capture the tap commit SHA for the summary:

```sh
TAP_SHA=$(gh api repos/DaveDev42/homebrew-tap/commits/main \
  --jq '.sha')
```

### Step 6 — Real `brew upgrade` smoke test

```sh
brew update
brew upgrade davedev42/tap/git-worktree-manager
gw --version
```

Verify `gw --version` output contains `<VERSION>`. Mismatch → abort
("installed version does not match released version — likely Homebrew
bottle cache is stale or the tap push is still in flight").

If the tap was not previously installed, `brew upgrade` will fail with
"No such keg". In that case run `brew install` instead. Re-run
`gw --version` afterward to confirm.

### Step 7 — Final summary

Print:

- Release PR: `#<num>` — `<title>` (merged at `<MERGE_SHA>`)
- Tag: `v<VERSION>`
- GitHub Release URL:
  `gh release view "v<VERSION>" --json url --jq '.url'`
- Tap commit:
  `https://github.com/DaveDev42/homebrew-tap/commit/<TAP_SHA>`
- Installed: output of `gw --version`

If any step was skipped (e.g. release PR was already merged when this
ran, so Steps 1-3 were no-ops), say so explicitly.

## Re-running

Re-running this command after a partial failure is safe and idempotent:

- Step 1 finds the existing release PR.
- Step 2's empty-rollup retrigger is no-op once `ci-gate` exists; if a
  prior run already pushed `chore: retrigger ci-gate` onto the PR
  branch, the `Test`-run query now returns rows and the skill skips
  retrigger automatically. The earlier empty commit disappears at
  squash-merge.
- Step 3 detects an already-merged PR via `gh pr view --json state` →
  `MERGED`, and skips merge.
- Step 4 finds the most recent run regardless of who triggered it.
- Steps 5-7 are read-only verification and always safe.
