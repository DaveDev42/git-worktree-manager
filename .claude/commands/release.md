---
description: Cut a patch release of git-worktree-manager (release-please + brew tap verified)
---

Drive a patch release end-to-end. The release pipeline is automated by
release-please + `.github/workflows/release.yml` (multi-platform build,
GitHub Release publish, Homebrew tap update, crates.io publish). Your job
is to gate, merge, watch, and **verify the tap actually got the new
version** — `release.yml` swallows tap-update warnings, so a "green"
workflow can still leave the tap one version behind.

Before starting, read `docs/release.md` once. It is the canonical
deep-dive on branch protection, the conventional-commit rules, and the
pre-release checklist. This command is the automation wrapper around it.

## Inputs

None. Always operates on `main`, always patch-bump (release-please
decides the version). Major/minor bumps are out of scope — see
`docs/release.md` "Manual major/minor bump".

## Procedure

Stop and report on any failure. Do **not** auto-recover.

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

Then run the drift check that CI does not cover:

```sh
cargo clippy --all-targets --all-features -- -D warnings
```

`docs/release.md` warns this combination catches drift the default CI
clippy run misses (`#[cfg(test)]` lints, feature-gated code, test/example
targets). `cargo test`, `cargo fmt --check`, and `cargo build --release`
are CI-covered — do **not** re-run them locally.

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

### Step 4 — Watch the `release.yml` run

Find the run triggered by the merge and watch it:

```sh
RUN_ID=$(gh run list --workflow=release.yml --branch=main --limit=1 \
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

- **On success**, additionally check for swallowed tap warnings:

  ```sh
  gh run view "$RUN_ID" --log | \
    grep -E "::warning::(HOMEBREW_TAP_TOKEN not set|Homebrew tap push failed)"
  ```

  If either warning matches, abort with the matched line. The release
  succeeded but the tap is out of sync — `release.yml` lines 131-134 and
  199 fall back to non-fatal warnings, which is the exact failure mode
  this command exists to catch.

### Step 5 — Tap repo sanity check

```sh
gh api repos/DaveDev42/homebrew-tap/commits/main \
  --jq '.commit.message'
```

Expected: `chore: update git-worktree-manager to <VERSION>` (matching
the `<VERSION>` from Step 1). If absent, mismatched, or stale, abort
with the actual message.

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
- Step 3 detects an already-merged PR via `gh pr view --json state` →
  `MERGED`, and skips merge.
- Step 4 finds the most recent run regardless of who triggered it.
- Steps 5-7 are read-only verification and always safe.
