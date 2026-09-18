use super::*;

#[test]
fn parse_toml_minimal() {
    let toml = r#"
name = "my-team"

[quality_gate]
check = "cargo check"
test = "cargo test"
"#;
    let frag = UserPresetFragment::from_toml(toml).unwrap();
    assert_eq!(frag.name, "my-team");
    assert_eq!(frag.layer_type, FragmentLayerType::ProjectLocal);
    assert_eq!(frag.merge_strategy, MergeStrategy::Replace);
    assert!(frag.extends.is_none());
    assert_eq!(
        frag.quality_gate.as_ref().unwrap().check.as_deref(),
        Some("cargo check")
    );
}

#[test]
fn parse_toml_full() {
    let toml = r#"
name = "acme-rust"
description = "Acme Corp Rust standards"
layer_type = "user-override"
extends = "rust"
merge_strategy = "append"
extra_allowed_tools = ["Bash(make:*)"]

[quality_gate]
check = "cargo check --workspace"
test = "cargo nextest run"
lint = "cargo clippy -- -D warnings"

[format]
command = "cargo fmt"
extensions = ["rs"]

[testing]
companion_pattern = "{name}.tests.{ext}"
test_dir = "alongside"
"#;
    let frag = UserPresetFragment::from_toml(toml).unwrap();
    assert_eq!(frag.name, "acme-rust");
    assert_eq!(
        frag.description.as_deref(),
        Some("Acme Corp Rust standards")
    );
    assert_eq!(frag.layer_type, FragmentLayerType::UserOverride);
    assert_eq!(frag.extends.as_deref(), Some("rust"));
    assert_eq!(frag.merge_strategy, MergeStrategy::Append);
    assert_eq!(frag.extra_allowed_tools, vec!["Bash(make:*)"]);
}

#[test]
fn parse_json_fragment() {
    let json = r#"{
        "name": "legacy",
        "quality_gate": { "check": "npm run build" }
    }"#;
    let frag = UserPresetFragment::from_json(json).unwrap();
    assert_eq!(frag.name, "legacy");
    assert_eq!(frag.layer_type, FragmentLayerType::ProjectLocal);
}

#[test]
fn roundtrip_toml() {
    let frag = UserPresetFragment {
        name: "test-roundtrip".into(),
        description: Some("A test".into()),
        layer_type: FragmentLayerType::ProjectLocal,
        extends: Some("python".into()),
        merge_strategy: MergeStrategy::Replace,
        quality_gate: Some(QualityGateOverride {
            check: Some("mypy .".into()),
            test: None,
            lint: None,
        }),
        format: None,
        testing: None,
        extra_allowed_tools: vec![],
    };

    let toml_str = frag.to_toml().unwrap();
    let parsed = UserPresetFragment::from_toml(&toml_str).unwrap();
    assert_eq!(frag, parsed);
}

#[test]
fn to_override_converts_correctly() {
    let frag = UserPresetFragment {
        name: "test".into(),
        description: None,
        layer_type: FragmentLayerType::ProjectLocal,
        extends: None,
        merge_strategy: MergeStrategy::Replace,
        quality_gate: Some(QualityGateOverride {
            check: Some("make check".into()),
            test: None,
            lint: None,
        }),
        format: None,
        testing: None,
        extra_allowed_tools: vec!["Read".into()],
    };

    let ov = frag.to_override();
    assert_eq!(ov.quality_gate.unwrap().check.unwrap(), "make check");
    assert_eq!(ov.extra_allowed_tools, vec!["Read"]);
}

#[test]
fn validate_empty_name() {
    let frag = UserPresetFragment {
        name: "  ".into(),
        description: None,
        layer_type: FragmentLayerType::ProjectLocal,
        extends: None,
        merge_strategy: MergeStrategy::Replace,
        quality_gate: Some(QualityGateOverride::default()),
        format: None,
        testing: None,
        extra_allowed_tools: vec![],
    };
    let warnings = frag.validate();
    assert!(warnings.iter().any(|w| w.is_error && w.field == "name"));
}

#[test]
fn validate_no_overrides() {
    let frag = UserPresetFragment {
        name: "empty".into(),
        description: None,
        layer_type: FragmentLayerType::ProjectLocal,
        extends: None,
        merge_strategy: MergeStrategy::Replace,
        quality_gate: None,
        format: None,
        testing: None,
        extra_allowed_tools: vec![],
    };
    let warnings = frag.validate();
    assert!(warnings.iter().any(|w| w.field == "(root)"));
}

#[test]
fn validate_empty_extends() {
    let frag = UserPresetFragment {
        name: "bad-extends".into(),
        description: None,
        layer_type: FragmentLayerType::ProjectLocal,
        extends: Some("".into()),
        merge_strategy: MergeStrategy::Replace,
        quality_gate: Some(QualityGateOverride::default()),
        format: None,
        testing: None,
        extra_allowed_tools: vec![],
    };
    assert!(frag.has_errors());
}

#[test]
fn validate_empty_gate_command_warns() {
    let frag = UserPresetFragment {
        name: "warn-gates".into(),
        description: None,
        layer_type: FragmentLayerType::ProjectLocal,
        extends: None,
        merge_strategy: MergeStrategy::Replace,
        quality_gate: Some(QualityGateOverride {
            check: Some("".into()),
            test: Some("".into()),
            lint: None,
        }),
        format: None,
        testing: None,
        extra_allowed_tools: vec![],
    };
    let warnings = frag.validate();
    let gate_warns: Vec<_> = warnings
        .iter()
        .filter(|w| w.field.starts_with("quality_gate"))
        .collect();
    assert_eq!(gate_warns.len(), 2);
    assert!(gate_warns.iter().all(|w| !w.is_error));
}

#[test]
fn load_from_toml_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("my-preset.toml");
    std::fs::write(
        &path,
        r#"
name = "file-test"
[quality_gate]
check = "echo ok"
"#,
    )
    .unwrap();

    let frag = UserPresetFragment::load(&path).unwrap();
    assert_eq!(frag.name, "file-test");
}

#[test]
fn load_from_json_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("my-preset.json");
    std::fs::write(
        &path,
        r#"{"name": "json-test", "quality_gate": {"check": "echo ok"}}"#,
    )
    .unwrap();

    let frag = UserPresetFragment::load(&path).unwrap();
    assert_eq!(frag.name, "json-test");
}

#[test]
fn save_and_reload() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("saved.toml");

    let frag = UserPresetFragment {
        name: "saved".into(),
        description: Some("Saved fragment".into()),
        layer_type: FragmentLayerType::UserOverride,
        extends: None,
        merge_strategy: MergeStrategy::Append,
        quality_gate: Some(QualityGateOverride {
            check: Some("make".into()),
            test: None,
            lint: None,
        }),
        format: None,
        testing: None,
        extra_allowed_tools: vec!["Write".into()],
    };

    frag.save(&path).unwrap();
    let loaded = UserPresetFragment::load(&path).unwrap();
    assert_eq!(frag, loaded);
}

#[test]
fn resolve_inheritance_rust() {
    let frag = UserPresetFragment {
        name: "custom-rust".into(),
        description: None,
        layer_type: FragmentLayerType::ProjectLocal,
        extends: Some("rust".into()),
        merge_strategy: MergeStrategy::Replace,
        quality_gate: Some(QualityGateOverride {
            check: Some("cargo check --all-features".into()),
            test: None,
            lint: None,
        }),
        format: None,
        testing: None,
        extra_allowed_tools: vec![],
    };

    let resolved = frag.resolve_inheritance().unwrap().unwrap();
    // Check command was overridden
    assert_eq!(resolved.quality_gate.check, "cargo check --all-features");
    // Test command inherited from rust preset
    assert!(!resolved.quality_gate.test.is_empty());
}

#[test]
fn resolve_inheritance_none() {
    let frag = UserPresetFragment {
        name: "no-inherit".into(),
        description: None,
        layer_type: FragmentLayerType::ProjectLocal,
        extends: None,
        merge_strategy: MergeStrategy::Replace,
        quality_gate: None,
        format: None,
        testing: None,
        extra_allowed_tools: vec![],
    };

    assert!(frag.resolve_inheritance().unwrap().is_none());
}

#[test]
fn resolve_inheritance_unknown_preset() {
    let frag = UserPresetFragment {
        name: "bad-extends".into(),
        description: None,
        layer_type: FragmentLayerType::ProjectLocal,
        extends: Some("brainfuck".into()),
        merge_strategy: MergeStrategy::Replace,
        quality_gate: None,
        format: None,
        testing: None,
        extra_allowed_tools: vec![],
    };

    assert!(frag.resolve_inheritance().is_err());
}
