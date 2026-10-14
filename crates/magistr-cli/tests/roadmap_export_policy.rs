//! Explicit CLI export honors the project's current export setting.

#[path = "support/magistr_command.rs"]
mod magistr_command;

#[test]
fn roadmap_exports_policy_blocks_export_before_opening_the_store() {
    let root = tempfile::tempdir().expect("project");
    std::fs::create_dir(root.path().join(".magistr")).expect("config directory");
    // Current settings disable export before any native store is opened.
    std::fs::write(
        root.path().join(".magistr/project.json"),
        serde_json::json!({
    "project": {"name": "settings-test", "type": "cli"},
    "stack": {"language": "unknown", "framework": "none", "package_manager": "unknown", "test_framework": "none", "linter": "none", "formatter": "none"},
    "quality_gate": {"check": "true", "test": "true", "lint": "true"},
    "format": {"command": "", "extensions": []},
    "testing": {"companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside"},
    "reference_repos": [], "state_prefix": "settings-test", "roadmap_exports": false
}).to_string(),
    ).expect("policy");
    let output = magistr_command::magistr_command()
        .args(["roadmap", "export"])
        .current_dir(root.path())
        .output()
        .expect("CLI");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("roadmap_exports: false"), "{stderr}");
    assert!(!root.path().join("ROADMAP.md").exists());
}
