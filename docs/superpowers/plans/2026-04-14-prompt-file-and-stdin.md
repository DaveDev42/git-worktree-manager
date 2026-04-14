# Prompt File & Stdin Support for `gw new` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `--prompt-file <path>` and `--prompt-stdin` flags to `gw new` so callers can pass initial AI prompts without shell-escaping pain, and update the `gw` skill to recommend and use `--prompt-file` as the default.

**Architecture:** Extend the `Commands::New` variant in `src/cli.rs` with two new optional flags grouped mutually-exclusive with `--prompt` via `clap`'s `ArgGroup`. In `src/main.rs`, resolve whichever source was provided into a single `Option<String>` before handing it to `worktree::create_worktree` — the downstream API is unchanged. Update the `gw` skill documentation embedded in `src/operations/setup_claude.rs` to document all three options and default to `--prompt-file` for skill-driven invocations.

**Tech Stack:** Rust, clap (derive + ArgGroup), std::fs / std::io, cargo test.

---

## File Structure

- **Modify** `src/cli.rs` — add `prompt_file: Option<PathBuf>` and `prompt_stdin: bool` to `Commands::New`, wire an `ArgGroup` so at most one of `--prompt | --prompt-file | --prompt-stdin` is used.
- **Modify** `src/main.rs` — in the `Commands::New` arm, resolve the three inputs into a single `Option<String>` and pass to `worktree::create_worktree`.
- **Modify** `src/operations/setup_claude.rs` — update the embedded skill markdown (`skill_content()`) and command reference (`reference_content()`) to document the three options and recommend `--prompt-file` for delegated tasks.
- **Modify** `tests/test_cli.rs` — add parse tests for the new flags and the mutual-exclusion group.
- **Create** `tests/test_prompt_sources.rs` — integration-style test for the resolution helper (if extracted) covering file, stdin, inline, and mutual exclusion.

Resolution logic lives in a small helper (`fn resolve_prompt(inline: Option<String>, file: Option<&Path>, stdin: bool) -> Result<Option<String>>`) in `src/main.rs` so it can be unit-tested directly.

---

### Task 1: Add `--prompt-file` and `--prompt-stdin` flags to the CLI

**Files:**
- Modify: `src/cli.rs:75-103` (the `Commands::New` variant)
- Test: `tests/test_cli.rs`

- [ ] **Step 1: Write the failing tests**

Append to `tests/test_cli.rs`:

```rust
use git_worktree_manager::cli::{Cli, Commands};
use clap::Parser;

#[test]
fn new_accepts_prompt_file_flag() {
    let cli = Cli::try_parse_from(["gw", "new", "feat-x", "--prompt-file", "/tmp/p.txt"])
        .expect("parses");
    let Some(Commands::New { prompt, prompt_file, prompt_stdin, .. }) = cli.command else {
        panic!("expected New variant");
    };
    assert!(prompt.is_none());
    assert_eq!(prompt_file.as_deref().and_then(|p| p.to_str()), Some("/tmp/p.txt"));
    assert!(!prompt_stdin);
}

#[test]
fn new_accepts_prompt_stdin_flag() {
    let cli = Cli::try_parse_from(["gw", "new", "feat-x", "--prompt-stdin"]).expect("parses");
    let Some(Commands::New { prompt_stdin, .. }) = cli.command else {
        panic!("expected New variant");
    };
    assert!(prompt_stdin);
}

#[test]
fn new_rejects_conflicting_prompt_sources() {
    let err = Cli::try_parse_from([
        "gw", "new", "feat-x",
        "--prompt", "hi",
        "--prompt-file", "/tmp/p.txt",
    ])
    .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("cannot be used with") || msg.contains("conflict"),
        "expected conflict error, got: {msg}"
    );
}

#[test]
fn new_rejects_prompt_and_stdin() {
    let err = Cli::try_parse_from(["gw", "new", "feat-x", "--prompt", "hi", "--prompt-stdin"])
        .unwrap_err();
    assert!(err.to_string().contains("cannot be used with") || err.to_string().contains("conflict"));
}
```

Note: if `Cli`/`Commands` aren't currently re-exported from the crate root, adjust the `use` to `use git_worktree_manager::cli::...` and confirm `lib.rs` exposes `pub mod cli;` (it does — see `src/lib.rs`).

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test test_cli -- new_accepts_prompt_file_flag new_accepts_prompt_stdin_flag new_rejects_conflicting_prompt_sources new_rejects_prompt_and_stdin`
Expected: FAIL — compile errors on unknown fields `prompt_file`, `prompt_stdin`.

- [ ] **Step 3: Add the new fields and the ArgGroup to `Commands::New`**

Edit `src/cli.rs`. At the top of the file, change the clap import line from:

```rust
use clap::{Args, Parser, Subcommand, ValueHint};
```

to (no change needed if `ArgGroup` is not referenced via derive attribute; the derive `group(...)` attribute works without importing `ArgGroup`). Keep the import as-is.

Also add `use std::path::PathBuf;` near the other imports (top of file, after the existing `use clap::...` line).

Replace the existing `New { ... }` variant (currently lines 75-103) with:

```rust
    /// Create new worktree for feature branch
    #[command(group(
        clap::ArgGroup::new("prompt_source")
            .args(["prompt", "prompt_file", "prompt_stdin"])
            .multiple(false)
            .required(false)
    ))]
    New {
        /// Branch name for the new worktree
        name: String,

        /// Custom worktree path (default: ../<repo>-<branch>)
        #[arg(short, long, value_hint = ValueHint::DirPath)]
        path: Option<String>,

        /// Base branch to create from (default: from config)
        #[arg(short = 'b', long = "base")]
        base: Option<String>,

        /// Skip AI tool launch
        #[arg(long = "no-term")]
        no_term: bool,

        /// Terminal launch method (e.g., tmux, iterm-tab, zellij)
        #[arg(short = 'T', long)]
        term: Option<String>,

        /// Launch AI tool in background
        #[arg(long)]
        bg: bool,

        /// Initial prompt to pass to the AI tool (starts interactive session with task)
        #[arg(long)]
        prompt: Option<String>,

        /// Read the initial prompt from a file (recommended for multi-line prompts)
        #[arg(long = "prompt-file", value_hint = ValueHint::FilePath)]
        prompt_file: Option<PathBuf>,

        /// Read the initial prompt from standard input
        #[arg(long = "prompt-stdin")]
        prompt_stdin: bool,
    },
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --test test_cli`
Expected: PASS (including the four new tests and the existing `clean_accepts_no_cache_flag`).

- [ ] **Step 5: Commit**

```bash
git add src/cli.rs tests/test_cli.rs
git commit -m "feat(cli): add --prompt-file and --prompt-stdin to gw new"
```

---

### Task 2: Resolve prompt source in `main.rs`

**Files:**
- Modify: `src/main.rs:88-109` (the `Commands::New` arm)
- Create: `tests/test_prompt_sources.rs`

- [ ] **Step 1: Write the failing test**

Create `tests/test_prompt_sources.rs`:

```rust
use git_worktree_manager::resolve_prompt;
use std::io::Write;
use std::path::PathBuf;

#[test]
fn resolve_prompt_returns_inline_when_only_inline_set() {
    let out = resolve_prompt(Some("hello".to_string()), None, false, || unreachable!()).unwrap();
    assert_eq!(out.as_deref(), Some("hello"));
}

#[test]
fn resolve_prompt_reads_file_contents_and_trims_trailing_newline() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p.txt");
    let mut f = std::fs::File::create(&path).unwrap();
    writeln!(f, "line1\nline2").unwrap();
    let out = resolve_prompt(None, Some(path.as_path()), false, || unreachable!()).unwrap();
    assert_eq!(out.as_deref(), Some("line1\nline2"));
}

#[test]
fn resolve_prompt_reads_from_stdin_reader() {
    let out = resolve_prompt(None, None, true, || Ok("piped content\n".to_string())).unwrap();
    assert_eq!(out.as_deref(), Some("piped content"));
}

#[test]
fn resolve_prompt_returns_none_when_no_source() {
    let out = resolve_prompt(None, None, false, || unreachable!()).unwrap();
    assert!(out.is_none());
}

#[test]
fn resolve_prompt_errors_when_file_missing() {
    let p = PathBuf::from("/nonexistent/definitely/not/here.txt");
    let err = resolve_prompt(None, Some(&p), false, || unreachable!()).unwrap_err();
    assert!(err.to_string().to_lowercase().contains("prompt"));
}
```

Ensure `tempfile` is a dev-dependency. Check `Cargo.toml`; if missing, add under `[dev-dependencies]`:

```toml
tempfile = "3"
```

(Only add if not already present — this project likely already uses it given the test count.)

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test test_prompt_sources`
Expected: FAIL — `resolve_prompt` is not defined / not exported from the crate.

- [ ] **Step 3: Add the helper and wire it into `main.rs`**

Edit `src/main.rs`. Near the top (after existing `use` statements), add:

```rust
use std::io::Read;
use std::path::Path;
```

(If `std::path::Path` is already imported transitively, skip the duplicate.)

Add the resolver function, placed near other free functions at the top level of `main.rs`:

```rust
/// Resolve the three mutually-exclusive prompt sources into a single optional string.
///
/// Exactly zero or one of (`inline`, `file`, `stdin`) should be active — clap's
/// ArgGroup enforces this at parse time. The `stdin_reader` closure exists so
/// tests can inject input without touching the real stdin.
///
/// The final string has a single trailing newline stripped if present.
pub fn resolve_prompt(
    inline: Option<String>,
    file: Option<&Path>,
    stdin: bool,
    stdin_reader: impl FnOnce() -> std::io::Result<String>,
) -> crate::error::Result<Option<String>> {
    let raw: Option<String> = if let Some(s) = inline {
        Some(s)
    } else if let Some(p) = file {
        Some(std::fs::read_to_string(p).map_err(|e| {
            crate::error::CwError::Other(format!(
                "failed to read --prompt-file '{}': {e}",
                p.display()
            ))
        })?)
    } else if stdin {
        Some(stdin_reader().map_err(|e| {
            crate::error::CwError::Other(format!("failed to read --prompt-stdin: {e}"))
        })?)
    } else {
        None
    };

    Ok(raw.map(|s| {
        // Strip a single trailing newline (common when piping or using text editors).
        let trimmed = s.strip_suffix('\n').unwrap_or(&s);
        let trimmed = trimmed.strip_suffix('\r').unwrap_or(trimmed);
        trimmed.to_string()
    }))
}
```

If `CwError` does not have an `Other(String)` variant, use whatever general-purpose variant the crate offers (inspect `src/error.rs` and use the closest match, e.g. `CwError::Custom(..)` or `CwError::Io(..)`). If a string variant does not exist, add `CwError::Other(String)` to `src/error.rs` with a `#[error("{0}")]` attribute.

To make the helper accessible from the integration test, export it from `src/lib.rs` by adding:

```rust
pub use crate::main_helpers::resolve_prompt;
```

— but `main.rs` is not part of the library crate. Preferred approach: move `resolve_prompt` into a new module `src/prompt_source.rs` and declare it in `src/lib.rs`:

Create `src/prompt_source.rs` with the helper body above (adjust `crate::error::...` accordingly since it's now the library crate). Then add to `src/lib.rs`:

```rust
pub mod prompt_source;
pub use prompt_source::resolve_prompt;
```

And in `src/main.rs`, import it:

```rust
use git_worktree_manager::resolve_prompt;
```

- [ ] **Step 4: Wire it into the `Commands::New` arm**

Edit `src/main.rs` around lines 88-109. Replace:

```rust
        Some(Commands::New {
            name,
            path,
            base,
            no_term,
            term,
            bg: _,
            prompt,
        }) => {
            // Prompt for .cwshare setup on first run
            cwshare_setup::prompt_cwshare_setup();

            worktree::create_worktree(
                &name,
                base.as_deref(),
                path.as_deref(),
                term.as_deref(),
                no_term,
                prompt.as_deref(),
            )
            .map(|_| ())
        }
```

with:

```rust
        Some(Commands::New {
            name,
            path,
            base,
            no_term,
            term,
            bg: _,
            prompt,
            prompt_file,
            prompt_stdin,
        }) => {
            cwshare_setup::prompt_cwshare_setup();

            let resolved = resolve_prompt(
                prompt,
                prompt_file.as_deref(),
                prompt_stdin,
                || {
                    let mut buf = String::new();
                    std::io::stdin().read_to_string(&mut buf)?;
                    Ok(buf)
                },
            )?;

            worktree::create_worktree(
                &name,
                base.as_deref(),
                path.as_deref(),
                term.as_deref(),
                no_term,
                resolved.as_deref(),
            )
            .map(|_| ())
        }
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --test test_prompt_sources && cargo test --test test_cli && cargo build`
Expected: PASS; clean build.

- [ ] **Step 6: Run the full suite to confirm nothing regressed**

Run: `cargo test && cargo clippy -- -D warnings && cargo fmt --check`
Expected: all tests pass, zero clippy warnings, formatting clean.

- [ ] **Step 7: Commit**

```bash
git add src/main.rs src/lib.rs src/prompt_source.rs tests/test_prompt_sources.rs Cargo.toml
git commit -m "feat(cli): resolve prompt from --prompt/--prompt-file/--prompt-stdin"
```

---

### Task 3: Update the `gw` skill to document and prefer `--prompt-file`

**Files:**
- Modify: `src/operations/setup_claude.rs:113-247` (the `skill_content()` string)
- Modify: `src/operations/setup_claude.rs:249+` (the `reference_content()` string — update the `gw new` entry)

- [ ] **Step 1: Update the skill delegation step to use `--prompt-file`**

In `skill_content()`, replace the Step 2 block (around lines 134-138):

```markdown
### Step 2: Confirm and execute
Show the user what you're about to run, then execute:
```bash
gw new <branch-name> -T <terminal-method> --prompt "<task description>"
```
```

with:

```markdown
### Step 2: Confirm and execute

Prefer `--prompt-file` for anything beyond a single short line — it avoids all
shell escaping issues with quotes, newlines, and special characters.

**Recommended (use this by default):**
```bash
# Write the full prompt to a temp file, then pass the path.
cat > /tmp/gw-prompt-$$.txt <<'PROMPT'
<task description — multi-line OK, quotes OK, no escaping needed>
PROMPT
gw new <branch-name> -T <terminal-method> --prompt-file /tmp/gw-prompt-$$.txt
rm -f /tmp/gw-prompt-$$.txt
```

**Short one-liner alternative:**
```bash
gw new <branch-name> -T <terminal-method> --prompt "<short task>"
```

**Piping from another command:**
```bash
generate-spec | gw new <branch-name> -T <terminal-method> --prompt-stdin
```

Only one of `--prompt`, `--prompt-file`, `--prompt-stdin` may be given per invocation.
```

- [ ] **Step 2: Update the "Delegate a task" section**

Replace the block around lines 175-184:

```markdown
## Delegate a task to a new worktree

```bash
gw new <branch-name> -T <terminal-method> --prompt "<task description>"
```

Example:
```bash
gw new fix-auth -T w-t --prompt "Fix JWT token expiration check in src/auth.rs"
```
```

with:

```markdown
## Delegate a task to a new worktree

Three ways to supply the initial prompt (mutually exclusive):

| Flag | When to use |
|------|-------------|
| `--prompt-file <path>` ⭐ | **Recommended.** Multi-line prompts, prompts with quotes/special chars, anything skill-generated. |
| `--prompt "<text>"` | Short single-line prompts only. |
| `--prompt-stdin` | Piping from another command (`cmd \| gw new ... --prompt-stdin`). |

Example (recommended):
```bash
cat > /tmp/gw-prompt.txt <<'PROMPT'
Fix JWT token expiration check in src/auth.rs.
Make sure to cover the "leeway" edge case and add a unit test.
PROMPT
gw new fix-auth -T w-t --prompt-file /tmp/gw-prompt.txt
```

Example (short form):
```bash
gw new fix-auth -T w-t --prompt "Fix JWT token expiration check"
```
```

- [ ] **Step 3: Update the Quick Reference table row**

Replace the row:

```markdown
| `gw new <branch> [--prompt "..."]` | Create worktree + optionally launch AI with task |
```

with:

```markdown
| `gw new <branch> [--prompt-file <path> \| --prompt "..." \| --prompt-stdin]` | Create worktree + optionally launch AI with task |
```

- [ ] **Step 4: Update the Guidelines bullet about `--prompt`**

Replace the two bullets around lines 238-241:

```markdown
- **Fire-and-forget**: Once a worktree task is spawned, you CANNOT stop it, send follow-up messages, or interact with it. The `--prompt` is the ONLY instruction the delegated instance receives. Therefore:
  - Make the `--prompt` comprehensive — include all requirements, constraints, and acceptance criteria upfront
```

with:

```markdown
- **Fire-and-forget**: Once a worktree task is spawned, you CANNOT stop it, send follow-up messages, or interact with it. The initial prompt is the ONLY instruction the delegated instance receives. Therefore:
  - Make the prompt comprehensive — include all requirements, constraints, and acceptance criteria upfront
  - Use `--prompt-file` for anything non-trivial so escaping does not silently corrupt the instructions
```

- [ ] **Step 5: Update `reference_content()` `gw new` section**

Locate the `### \`gw new <branch> [OPTIONS]\`` block (around lines 256-264). Replace the existing `--prompt` line:

```markdown
- `--prompt <PROMPT>` — Initial prompt to pass to AI tool (interactive session)
```

with:

```markdown
- `--prompt <PROMPT>` — Initial prompt as a CLI string (single-line, best for short prompts)
- `--prompt-file <PATH>` — Read initial prompt from a file (recommended for multi-line / quoted content)
- `--prompt-stdin` — Read initial prompt from standard input (for piping)

Only one of `--prompt`, `--prompt-file`, `--prompt-stdin` may be used per invocation.
```

- [ ] **Step 6: Verify the skill content still compiles and renders**

Run: `cargo build && cargo test --test test_operations 2>/dev/null || cargo test`
Expected: build succeeds; no test regressions. The skill content is a raw string, so compilation is the main check.

Sanity-print the skill to confirm markdown looks right:

Run: `cargo run -- setup-claude --help 2>&1 | head -5`
Expected: help text for `setup-claude` (no crash).

- [ ] **Step 7: Commit**

```bash
git add src/operations/setup_claude.rs
git commit -m "docs(skill): document --prompt-file and --prompt-stdin in gw skill"
```

---

### Task 4: README / user-facing docs sweep

**Files:**
- Modify: `README.md` (search for `--prompt` and update)

- [ ] **Step 1: Search for references**

Run: `rg -n --fixed-strings -- "--prompt" README.md`
Expected: list of lines mentioning `--prompt`.

- [ ] **Step 2: Update each hit**

For each occurrence, extend the documentation to mention `--prompt-file` (preferred for multi-line) and `--prompt-stdin`. Mirror the table format from Task 3 Step 2. If a line is just a short-form example, leave `--prompt` as-is and add a follow-up example using `--prompt-file`.

If `rg` returns no hits, skip this task entirely (skip to Step 4 commit no-op, i.e. don't commit).

- [ ] **Step 3: Verify**

Run: `cargo build && rg -n --fixed-strings -- "--prompt-file" README.md`
Expected: build passes; at least one match of `--prompt-file` in README (if README had prompt mentions).

- [ ] **Step 4: Commit (only if README changed)**

```bash
git add README.md
git commit -m "docs(readme): document --prompt-file and --prompt-stdin"
```

---

### Task 5: End-to-end smoke test

**Files:**
- No new files; shell-only verification.

- [ ] **Step 1: Build release binary**

Run: `cargo build --release`
Expected: `target/release/gw` produced, no warnings.

- [ ] **Step 2: Verify help text advertises the new flags**

Run: `./target/release/gw new --help`
Expected: output contains `--prompt-file <PROMPT_FILE>` and `--prompt-stdin`, and the conflict metadata (clap shows "cannot be used with" when invoked with two sources).

- [ ] **Step 3: Verify conflict rejection**

Run: `./target/release/gw new test-branch --prompt hi --prompt-file /tmp/nope.txt 2>&1; echo "exit=$?"`
Expected: non-zero exit, error message referring to conflicting flags. Do not actually create a worktree.

- [ ] **Step 4: Verify file-not-found is a clean error**

Run: `./target/release/gw new test-branch --prompt-file /definitely/not/here.txt --no-term 2>&1; echo "exit=$?"`
Expected: non-zero exit; error message mentions `--prompt-file` and the path. No worktree created (or if one was created before the prompt read, investigate and fix the ordering — prompt resolution must happen before side effects). **If a worktree is created**, move the `resolve_prompt(...)?` call in `main.rs` above any side-effecting call (it already is, above `worktree::create_worktree`, so this should not regress).

- [ ] **Step 5: No commit (smoke test only)**

Nothing to commit — this is a manual verification task.

---

## Self-Review Checklist (executed inline)

**Spec coverage:**
- `--prompt-file` flag: Task 1 adds it; Task 2 consumes it. ✓
- `--prompt-stdin` flag: Task 1 adds it; Task 2 consumes it via injected reader. ✓
- Mutual exclusion: Task 1 ArgGroup; Task 1 Step 1 tests it. ✓
- Skill documents all three, recommends `--prompt-file`: Task 3. ✓
- Skill uses `--prompt-file` by default for automation: Task 3 Step 1. ✓

**Placeholder scan:** No TBD / TODO / "add error handling" / "similar to above" left. All code blocks are concrete.

**Type consistency:**
- `resolve_prompt` signature used identically in test (Task 2 Step 1) and impl (Task 2 Step 3). ✓
- `Commands::New` new fields (`prompt_file: Option<PathBuf>`, `prompt_stdin: bool`) referenced identically across cli.rs, main.rs, and test. ✓
- Error variant `CwError::Other(String)` is conditional — Task 2 Step 3 instructs the engineer to inspect `src/error.rs` and adapt.
