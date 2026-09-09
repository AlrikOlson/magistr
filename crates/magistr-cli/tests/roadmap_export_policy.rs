//! Explicit CLI exports honor the same project opt-out as MCP and legacy GUI.

#[path = "support/magistr_command.rs"]
mod magistr_command;

#[test]
fn roadmap_exports_policy_blocks_export_and_sync_before_opening_the_store() {
    let root = tempfile::tempdir().expect("project");
    std::fs::create_dir(root.path().join(".magistr")).expect("config directory");
    // Intentionally no native store or full workflow configuration: policy is
    // checked before those can be opened or any roadmap can be rendered.
    std::fs::write(
        root.path().join(".magistr/project.json"),
        r#"{"roadmap_exports":false}"#,
    )
    .expect("policy");
    for action in ["export", "sync"] {
        let output = magistr_command::magistr_command()
            .args(["roadmap", action])
            .current_dir(root.path())
            .output()
            .expect("CLI");
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("roadmap_exports: false"), "{stderr}");
        assert!(!root.path().join("ROADMAP.md").exists());
    }
}
