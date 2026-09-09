use super::*;

#[test]
fn no_user_preset_returns_none() {
    let dir = tempfile::TempDir::new().unwrap();
    let result = load_user_override_from_dir(dir.path(), Language::Rust).unwrap();
    assert!(result.is_none());
}

#[test]
fn loads_valid_user_preset() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("rust.json"),
        r#"{
                "quality_gate": {
                    "check": "cargo check --all-features"
                }
            }"#,
    )
    .unwrap();

    let result = load_user_override_from_dir(dir.path(), Language::Rust).unwrap();
    assert!(result.is_some());
    let preset = result.unwrap();
    assert_eq!(
        preset.quality_gate.as_ref().unwrap().check.as_deref(),
        Some("cargo check --all-features")
    );
    // Unset fields remain None
    assert!(preset.quality_gate.as_ref().unwrap().test.is_none());
    assert!(preset.format.is_none());
}

#[test]
fn invalid_json_returns_error() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("rust.json"), "not valid json").unwrap();

    let result = load_user_override_from_dir(dir.path(), Language::Rust);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("invalid user preset"));
}

#[test]
fn ignores_wrong_language() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("python.json"),
        r#"{"quality_gate": {"test": "pytest -x"}}"#,
    )
    .unwrap();

    // Looking for Rust should return None even though python.json exists
    let result = load_user_override_from_dir(dir.path(), Language::Rust).unwrap();
    assert!(result.is_none());
}

#[test]
fn list_presets_empty_dir() {
    let dir = tempfile::TempDir::new().unwrap();
    let presets = list_user_presets_in_dir(dir.path());
    assert!(presets.is_empty());
}

#[test]
fn list_presets_finds_json_files() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("rust.json"), "{}").unwrap();
    std::fs::write(dir.path().join("python.json"), "{}").unwrap();
    std::fs::write(dir.path().join("readme.txt"), "ignore me").unwrap();

    let presets = list_user_presets_in_dir(dir.path());
    assert_eq!(presets.len(), 2);
    assert!(presets.iter().all(|p| p.extension().unwrap() == "json"));
}

#[test]
fn missing_dir_returns_none() {
    let result =
        load_user_override_from_dir(Path::new("/nonexistent/path"), Language::Rust).unwrap();
    assert!(result.is_none());
}

#[test]
fn full_override_all_fields() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("typescript.json"),
        r#"{
                "quality_gate": {
                    "check": "tsc --noEmit",
                    "test": "vitest run",
                    "lint": "biome check"
                },
                "format": {
                    "extensions": ["ts", "tsx"],
                    "command": "biome format --write"
                },
                "testing": {
                    "companion_pattern": "{name}.spec.ts",
                    "test_dir": "src/__tests__/"
                },
                "extra_allowed_tools": ["Bash(npm:*)"]
            }"#,
    )
    .unwrap();

    let result = load_user_override_from_dir(dir.path(), Language::TypeScript).unwrap();
    let preset = result.unwrap();
    assert_eq!(
        preset.quality_gate.as_ref().unwrap().lint.as_deref(),
        Some("biome check")
    );
    assert_eq!(
        preset.format.as_ref().unwrap().command.as_deref(),
        Some("biome format --write")
    );
    assert_eq!(preset.extra_allowed_tools, vec!["Bash(npm:*)"]);
}
