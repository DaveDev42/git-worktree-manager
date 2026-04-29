//! Failing tests that pin the new `gw clean -i` redirect behavior.
//!
//! Both tests are intentionally written against the *future* state of the code:
//! - `gw clean -i` should exit non-zero and mention `gw delete -i`
//! - `gw clean --help` should mention `gw delete -i` in its output
//!
//! These tests FAIL against current `main` (where `clean -i` still works) and
//! will only pass after `clean.rs` and `cli.rs` are updated in later tasks.

mod common;
use common::TestRepo;

/// After the consolidation, `gw clean -i` must be removed.  The flag should
/// either be rejected by clap (unknown flag, exit 2) or produce a hard error
/// from the handler — either way the process must exit non-zero and the
/// combined output must contain the literal string `gw delete -i` so the user
/// knows where to go.
#[test]
fn clean_interactive_is_removed_and_points_at_delete() {
    let repo = TestRepo::new();
    let output = repo.cw(&["clean", "-i"]);

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(
        !output.status.success(),
        "gw clean -i should exit non-zero after the consolidation; exit code: {:?}\noutput: {}",
        output.status.code(),
        combined
    );

    assert!(
        combined.contains("gw delete -i"),
        "gw clean -i output should mention 'gw delete -i' to redirect the user; got:\n{}",
        combined
    );
}

/// After the consolidation, `gw clean --help` must surface a note pointing
/// users at `gw delete -i` for interactive deletion.  The doc-comment on
/// `Commands::Clean` will be updated in a later task; this test pins that
/// requirement now.
#[test]
fn clean_help_mentions_delete_i_redirect() {
    let repo = TestRepo::new();
    let output = repo.cw(&["clean", "--help"]);

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();

    assert!(
        output.status.success(),
        "gw clean --help should exit 0; got: {:?}\nstdout: {}\nstderr: {}",
        output.status.code(),
        stdout,
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(
        stdout.contains("gw delete -i"),
        "gw clean --help should mention 'gw delete -i' for interactive use; got:\n{}",
        stdout
    );
}
