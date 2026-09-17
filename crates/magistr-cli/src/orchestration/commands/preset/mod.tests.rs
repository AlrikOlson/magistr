use super::*;
use crate::config::preset_fragment::{FragmentLayerType, MergeStrategy, UserPresetFragment};
use crate::types::QualityGateOverride;

#[test]
fn preset_test_valid_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test-preset.toml");
    let frag = UserPresetFragment {
        name: "test-valid".into(),
        description: Some("A test preset".into()),
        layer_type: FragmentLayerType::ProjectLocal,
        extends: None,
        merge_strategy: MergeStrategy::Replace,
        quality_gate: Some(QualityGateOverride {
            check: Some("echo check".into()),
            test: Some("echo test".into()),
            lint: None,
        }),
        format: None,
        testing: None,
        extra_allowed_tools: vec![],
    };
    frag.save(&path).unwrap();

    // Should not error
    cmd_preset_test(&path).unwrap();
}

#[test]
fn preset_test_invalid_name() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.toml");
    std::fs::write(
        &path,
        r#"
name = ""
[quality_gate]
check = "echo ok"
"#,
    )
    .unwrap();

    // Should error due to empty name
    assert!(cmd_preset_test(&path).is_err());
}

#[test]
fn preset_test_with_extends() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("extended.toml");
    std::fs::write(
        &path,
        r#"
name = "my-rust"
extends = "rust"

[quality_gate]
check = "cargo check --all-features"
"#,
    )
    .unwrap();

    cmd_preset_test(&path).unwrap();
}

#[test]
fn preset_list_includes_builtins() {
    let dir = tempfile::tempdir().unwrap();
    // Create a mock renderer for testing
    let renderer = crate::output::PlainRenderer;

    // Should not error — just listing presets
    let inputs = crate::config::inputs::ProjectInputs::from_lookup(dir.path(), |_| None);
    cmd_preset_list(dir.path(), &renderer, &inputs).unwrap();
}
