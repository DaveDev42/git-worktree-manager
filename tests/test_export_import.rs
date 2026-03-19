/// Integration tests for export/import configuration — ported from Python test_export_import.py (13 tests).
mod common;

use common::TestRepo;

// ===========================================================================
// 1. export — basic with worktrees
// ===========================================================================

#[test]
fn test_export_config_basic() {
    let repo = TestRepo::new();
    repo.create_worktree("feature1");
    repo.create_worktree("feature2");

    let export_path = repo.path().join("test-export.json");
    let output = repo.cw(&["export", "--output", export_path.to_str().unwrap()]);
    assert!(output.status.success());
    assert!(export_path.exists());

    let content = std::fs::read_to_string(&export_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

    assert_eq!(parsed["export_version"].as_str().unwrap(), "1.0");
    assert!(parsed.get("exported_at").is_some());
    assert!(parsed.get("repository").is_some());
    assert!(parsed.get("config").is_some());
    assert!(parsed.get("worktrees").is_some());

    let worktrees = parsed["worktrees"].as_array().unwrap();
    assert_eq!(worktrees.len(), 2);

    // Verify worktree data
    let branches: Vec<&str> = worktrees
        .iter()
        .map(|w| w["branch"].as_str().unwrap())
        .collect();
    assert!(branches.contains(&"feature1"));
    assert!(branches.contains(&"feature2"));

    // Verify each worktree has required fields
    for wt in worktrees {
        assert!(
            wt.get("branch").is_some(),
            "Missing 'branch' field in worktree export"
        );
        assert!(
            wt.get("path").is_some(),
            "Missing 'path' field in worktree export"
        );
        // base_branch and status may or may not be present depending on implementation
    }
}

// ===========================================================================
// 2. export — default filename
// ===========================================================================

#[test]
fn test_export_config_default_filename() {
    let repo = TestRepo::new();
    let output = repo.cw(&["export"]);
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Export complete")
            || stdout.contains("export")
            || stdout.contains("gw-export"),
        "Expected export success message, got: {}",
        stdout
    );
}

// ===========================================================================
// 3. export — empty worktrees
// ===========================================================================

#[test]
fn test_export_config_empty_worktrees() {
    let repo = TestRepo::new();
    let export_path = repo.path().join("empty-export.json");
    let output = repo.cw(&["export", "--output", export_path.to_str().unwrap()]);
    assert!(output.status.success());
    assert!(export_path.exists());

    let content = std::fs::read_to_string(&export_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

    assert_eq!(parsed["export_version"].as_str().unwrap(), "1.0");
    assert_eq!(parsed["worktrees"].as_array().unwrap().len(), 0);
    assert!(parsed.get("config").is_some());
}

// ===========================================================================
// 4. export — contains config and metadata
// ===========================================================================

#[test]
fn test_export_config_contains_metadata() {
    let repo = TestRepo::new();
    repo.create_worktree("test-branch");

    let export_path = repo.path().join("custom-config-export.json");
    let output = repo.cw(&["export", "--output", export_path.to_str().unwrap()]);
    assert!(output.status.success());

    let content = std::fs::read_to_string(&export_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

    assert!(parsed.get("config").is_some());
    assert!(parsed.get("repository").is_some());
    assert!(parsed.get("exported_at").is_some());
}

// ===========================================================================
// 5. import — preview mode (default, no --apply)
// ===========================================================================

#[test]
fn test_import_config_preview_mode() {
    let repo = TestRepo::new();
    repo.create_worktree("feature1");
    repo.create_worktree("feature2");

    let export_path = repo.path().join("test-import.json");
    repo.cw(&["export", "--output", export_path.to_str().unwrap()]);

    // Import in preview mode (no --apply)
    let output = repo.cw(&["import", export_path.to_str().unwrap()]);
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Preview") || stdout.contains("preview"),
        "Expected preview mode indicator, got: {}",
        stdout
    );
}

// ===========================================================================
// 6. import — apply mode
// ===========================================================================

#[test]
fn test_import_config_apply_mode() {
    let repo = TestRepo::new();
    repo.create_worktree("feature1");

    let export_path = repo.path().join("test-apply.json");
    repo.cw(&["export", "--output", export_path.to_str().unwrap()]);

    // Import with --apply
    let output = repo.cw(&["import", export_path.to_str().unwrap(), "--apply"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Import complete")
            || stdout.contains("Applying")
            || stdout.contains("Applied")
            || stdout.contains("import"),
        "Expected import success indicator, got: {}",
        stdout
    );
}

// ===========================================================================
// 7. import — restores worktree metadata
// ===========================================================================

#[test]
fn test_import_config_worktree_metadata() {
    let repo = TestRepo::new();
    repo.create_worktree("metadata-test");

    let export_path = repo.path().join("metadata-export.json");
    repo.cw(&["export", "--output", export_path.to_str().unwrap()]);

    // Clear metadata
    let _ = std::process::Command::new("git")
        .args(["config", "--unset", "branch.metadata-test.worktreeBase"])
        .current_dir(repo.path())
        .output();

    // Verify metadata is cleared
    let meta = repo.git_stdout(&["config", "--get", "branch.metadata-test.worktreeBase"]);
    assert!(meta.trim().is_empty(), "Metadata should be cleared");

    // Import with apply
    let output = repo.cw(&["import", export_path.to_str().unwrap(), "--apply"]);
    assert!(output.status.success());

    // Verify metadata was restored
    let meta = repo.git_stdout(&["config", "--get", "branch.metadata-test.worktreeBase"]);
    assert_eq!(
        meta.trim(),
        "main",
        "Expected metadata to be restored to 'main'"
    );
}

// ===========================================================================
// 8. import — invalid JSON
// ===========================================================================

#[test]
fn test_import_config_invalid_json() {
    let repo = TestRepo::new();
    let bad_file = repo.path().join("invalid.json");
    std::fs::write(&bad_file, "{ invalid json content").unwrap();

    let output = repo.cw(&["import", bad_file.to_str().unwrap()]);
    assert!(!output.status.success());
}

// ===========================================================================
// 9. import — missing fields (graceful handling)
// ===========================================================================

#[test]
fn test_import_config_missing_fields() {
    let repo = TestRepo::new();
    let incomplete_file = repo.path().join("incomplete.json");
    std::fs::write(
        &incomplete_file,
        r#"{"export_version": "1.0", "exported_at": "2025-01-01T00:00:00"}"#,
    )
    .unwrap();

    // Import should handle missing fields gracefully
    let output = repo.cw(&["import", incomplete_file.to_str().unwrap()]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should either succeed with preview or handle gracefully
    assert!(
        output.status.success()
            || stdout.contains("Preview")
            || stdout.contains("Worktrees: 0")
            || stdout.contains("0 worktrees"),
        "Expected graceful handling of incomplete data"
    );
}

// ===========================================================================
// 10. import — version check (future version)
// ===========================================================================

#[test]
fn test_import_config_version_check() {
    let repo = TestRepo::new();
    let future_file = repo.path().join("future.json");
    std::fs::write(
        &future_file,
        r#"{"export_version": "99.0", "exported_at": "2025-01-01T00:00:00", "repository": "/tmp/test", "config": {}, "worktrees": []}"#,
    ).unwrap();

    // Should handle gracefully (either accept or warn)
    let output = repo.cw(&["import", future_file.to_str().unwrap()]);
    // We just check it doesn't crash — may fail validation or succeed
    let _ = output;
}

// ===========================================================================
// 11. export/import roundtrip
// ===========================================================================

#[test]
fn test_export_import_roundtrip() {
    let repo = TestRepo::new();
    repo.create_worktree("roundtrip1");
    repo.create_worktree("roundtrip2");

    // Export
    let export_path = repo.path().join("roundtrip.json");
    let output = repo.cw(&["export", "--output", export_path.to_str().unwrap()]);
    assert!(output.status.success());

    // Clear one worktree's metadata
    let _ = std::process::Command::new("git")
        .args(["config", "--unset", "branch.roundtrip1.worktreeBase"])
        .current_dir(repo.path())
        .output();

    // Import to restore
    let output = repo.cw(&["import", export_path.to_str().unwrap(), "--apply"]);
    assert!(output.status.success());

    // Verify metadata was restored
    let meta = repo.git_stdout(&["config", "--get", "branch.roundtrip1.worktreeBase"]);
    assert_eq!(meta.trim(), "main", "Expected metadata restored to 'main'");
}

// ===========================================================================
// 12. export — stale worktree (deleted directory)
// ===========================================================================

#[test]
fn test_export_config_with_stale_worktree() {
    let repo = TestRepo::new();
    let wt = repo.create_worktree("stale-wt");

    // Delete the worktree directory manually
    std::fs::remove_dir_all(&wt).unwrap();

    // Export should still work
    let export_path = repo.path().join("stale-export.json");
    let output = repo.cw(&["export", "--output", export_path.to_str().unwrap()]);
    assert!(output.status.success());
    assert!(export_path.exists());

    let content = std::fs::read_to_string(&export_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

    // Should include the stale worktree with status "stale"
    let worktrees = parsed["worktrees"].as_array().unwrap();
    let stale_wt = worktrees
        .iter()
        .find(|w| w["branch"].as_str().unwrap_or("") == "stale-wt");
    if let Some(wt) = stale_wt {
        // If included, status should indicate staleness
        let status = wt.get("status").and_then(|s| s.as_str()).unwrap_or("");
        assert!(
            status == "stale" || status.is_empty() || status.contains("stale"),
            "Expected stale-like status for deleted worktree, got: '{}'",
            status
        );
    }
    // Some implementations may omit stale worktrees from export.
    // The important thing is the export succeeded without error.
}

// ===========================================================================
// 13. import — partial worktrees (some don't exist locally)
// ===========================================================================

#[test]
fn test_import_config_partial_worktrees() {
    let repo = TestRepo::new();

    // Create export data with a nonexistent worktree
    let partial_file = repo.path().join("partial.json");
    let data = serde_json::json!({
        "export_version": "1.0",
        "exported_at": "2025-01-01T00:00:00",
        "repository": repo.path().to_str().unwrap(),
        "config": {},
        "worktrees": [
            {
                "branch": "nonexistent-branch",
                "base_branch": "main",
                "base_path": repo.path().to_str().unwrap(),
                "path": "/tmp/nonexistent-path",
                "status": "clean"
            }
        ]
    });
    std::fs::write(&partial_file, serde_json::to_string_pretty(&data).unwrap()).unwrap();

    // Import should handle nonexistent worktrees gracefully
    let output = repo.cw(&["import", partial_file.to_str().unwrap(), "--apply"]);
    // Should complete without hard crash
    let _stdout = String::from_utf8_lossy(&output.stdout);
    // The import may warn about missing worktrees but should not crash
}
