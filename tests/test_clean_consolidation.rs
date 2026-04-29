//! Integration tests pinning the `gw clean -i` redirect to `gw delete -i`.

mod common;
use common::TestRepo;

/// `gw clean -i` must exit non-zero and mention `gw delete -i` in its output
/// so users know where the interactive flow moved to.
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

/// `gw clean --help` must mention `gw delete -i` so users discover the
/// interactive flow even when reading help text.
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
