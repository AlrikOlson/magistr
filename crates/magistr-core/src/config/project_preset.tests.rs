use super::*;

#[test]
fn missing_dir_returns_empty() {
    let dir = std::path::Path::new("/nonexistent/path/.magistr/presets");
    let result = load_project_presets_from_dir(dir).unwrap();
    assert!(result.is_empty());
}

#[test]
fn empty_dir_returns_empty() {
    let dir = tempfile::TempDir::new().unwrap();
    let result = load_project_presets_from_dir(dir.path()).unwrap();
    assert!(result.is_empty());
}

#[test]
fn loads_single_preset() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("custom-db.json"),
        r#"{"extra_allowed_tools": ["Bash(redis-cli:*)"]}"#,
    )
    .unwrap();

    let layers = load_project_presets_from_dir(dir.path()).unwrap();
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].name, "custom-db");
    assert_eq!(layers[0].layer_type, PresetLayerType::ProjectLocal);
    assert!(
        layers[0]
            .overrides
            .extra_allowed_tools
            .contains(&"Bash(redis-cli:*)".to_string())
    );
}

#[test]
fn loads_multiple_presets_sorted() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("beta.json"),
        r#"{"extra_allowed_tools": ["Bash(beta:*)"]}"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("alpha.json"),
        r#"{"extra_allowed_tools": ["Bash(alpha:*)"]}"#,
    )
    .unwrap();

    let layers = load_project_presets_from_dir(dir.path()).unwrap();
    assert_eq!(layers.len(), 2);
    // Sorted alphabetically
    assert_eq!(layers[0].name, "alpha");
    assert_eq!(layers[1].name, "beta");
}

#[test]
fn ignores_non_json_files() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("readme.txt"), "ignore me").unwrap();
    std::fs::write(
        dir.path().join("valid.json"),
        r#"{"extra_allowed_tools": []}"#,
    )
    .unwrap();

    let layers = load_project_presets_from_dir(dir.path()).unwrap();
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].name, "valid");
}

#[test]
fn invalid_json_returns_error() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("bad.json"), "not json").unwrap();

    let result = load_project_presets_from_dir(dir.path());
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("invalid project preset"));
}

#[test]
fn load_from_project_root() {
    let root = tempfile::TempDir::new().unwrap();
    let preset_dir = root.path().join(".magistr/presets");
    std::fs::create_dir_all(&preset_dir).unwrap();
    std::fs::write(
        preset_dir.join("local.json"),
        r#"{"quality_gate": {"check": "custom check"}}"#,
    )
    .unwrap();

    let layers = load_project_presets(root.path()).unwrap();
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].name, "local");
    assert_eq!(
        layers[0]
            .overrides
            .quality_gate
            .as_ref()
            .unwrap()
            .check
            .as_deref(),
        Some("custom check")
    );
}

#[test]
fn load_from_project_root_no_dir() {
    let root = tempfile::TempDir::new().unwrap();
    let layers = load_project_presets(root.path()).unwrap();
    assert!(layers.is_empty());
}
