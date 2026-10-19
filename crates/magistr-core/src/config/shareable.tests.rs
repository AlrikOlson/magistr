use super::*;
use crate::types::*;

fn sample_project_config() -> ProjectConfig {
    ProjectConfig {
        project: ProjectMeta {
            id: None,
            name: "test-project".into(),
            description: "A test project".into(),
            project_type: "cli".into(),
        },
        stack: StackConfig {
            language: "rust".into(),
            framework: "none".into(),
            package_manager: "cargo".into(),
            test_framework: "cargo-test".into(),
            linter: "clippy".into(),
            formatter: "rustfmt".into(),
        },
        quality_gate: QualityGate {
            check: "cargo check --all-targets".into(),
            test: "cargo test".into(),
            lint: "cargo clippy -- -D warnings".into(),
        },
        format: FormatConfig {
            command: "cargo fmt".into(),
            extensions: vec![".rs".into()],
            exclusions: vec![],
        },
        spec: Some(SpecConfig {
            file: "ROADMAP.md".into(),
            unchecked_pattern: "- [ ]".into(),
        }),
        testing: TestingConfig {
            companion_pattern: "tests/{{name}}_test.rs".into(),
            test_dir: "tests".into(),
        },
        reference_repos: vec![],
        state_prefix: "test-project".into(),
        custom_gates: vec![],
        guards: Default::default(),
        hints: vec![],
        lifecycle_hooks: vec![],
        roadmap_backend: Default::default(),
        roadmap_exports: true,
        archetypes: vec![],
        topology: None,
        relationships: vec![],
    }
}

fn sample_preset() -> ShareablePreset {
    ShareablePreset {
        name: "test-project".into(),
        description: Some("A test project".into()),
        stack: ShareableStack {
            language: "rust".into(),
            framework: "none".into(),
            package_manager: "cargo".into(),
            test_framework: "cargo-test".into(),
            linter: "clippy".into(),
            formatter: "rustfmt".into(),
        },
        quality_gate: ShareableQualityGate {
            check: "cargo check --all-targets".into(),
            test: "cargo test".into(),
            lint: "cargo clippy -- -D warnings".into(),
        },
        format: ShareableFormat {
            command: "cargo fmt".into(),
            extensions: vec![".rs".into()],
        },
        testing: ShareableTesting {
            companion_pattern: "tests/{{name}}_test.rs".into(),
            test_dir: "tests".into(),
        },
    }
}

#[test]
fn from_project_config_captures_all_fields() {
    let config = sample_project_config();
    let preset = ShareablePreset::from_project_config(&config);
    assert_eq!(preset.name, "test-project");
    assert_eq!(preset.description, Some("A test project".into()));
    assert_eq!(preset.stack.language, "rust");
    assert_eq!(preset.stack.framework, "none");
    assert_eq!(preset.stack.package_manager, "cargo");
    assert_eq!(preset.stack.test_framework, "cargo-test");
    assert_eq!(preset.stack.linter, "clippy");
    assert_eq!(preset.stack.formatter, "rustfmt");
    assert_eq!(preset.quality_gate.check, "cargo check --all-targets");
    assert_eq!(preset.quality_gate.test, "cargo test");
    assert_eq!(preset.quality_gate.lint, "cargo clippy -- -D warnings");
    assert_eq!(preset.format.command, "cargo fmt");
    assert_eq!(preset.format.extensions, vec![".rs"]);
    assert_eq!(preset.testing.companion_pattern, "tests/{{name}}_test.rs");
    assert_eq!(preset.testing.test_dir, "tests");
}

#[test]
fn from_project_config_empty_description_becomes_none() {
    let mut config = sample_project_config();
    config.project.description = String::new();
    let preset = ShareablePreset::from_project_config(&config);
    assert!(preset.description.is_none());
}

#[test]
fn toml_round_trip() {
    let preset = sample_preset();
    let toml_str = preset.to_toml().expect("serialize");
    let parsed = ShareablePreset::from_toml(&toml_str).expect("parse");
    assert_eq!(preset, parsed);
}

#[test]
fn toml_output_is_readable() {
    let preset = sample_preset();
    let toml_str = preset.to_toml().expect("serialize");
    assert!(toml_str.contains("name = \"test-project\""));
    assert!(toml_str.contains("[stack]"));
    assert!(toml_str.contains("[quality_gate]"));
    assert!(toml_str.contains("[format]"));
    assert!(toml_str.contains("[testing]"));
}

#[test]
fn from_toml_invalid_returns_error() {
    let result = ShareablePreset::from_toml("not valid { toml }}}");
    assert!(result.is_err());
}

#[test]
fn from_toml_missing_required_field_returns_error() {
    let result = ShareablePreset::from_toml("name = \"test\"\n");
    assert!(result.is_err());
}

#[test]
fn save_and_load_round_trip() {
    let preset = sample_preset();
    let tmp = tempfile::TempDir::new().unwrap();
    let path = tmp.path().join("test-preset.toml");

    save_to_file(&path, &preset).expect("save");
    let loaded = load_from_file(&path).expect("load");
    assert_eq!(preset, loaded);
}

#[test]
fn load_nonexistent_file_returns_error() {
    let result = load_from_file(Path::new("/tmp/nonexistent-preset-file.toml"));
    assert!(result.is_err());
}

#[test]
fn to_template_vars_produces_expected_keys() {
    let preset = sample_preset();
    let vars = preset.to_template_vars("my-project");

    assert_eq!(vars.get("project_name").unwrap(), "my-project");
    assert_eq!(vars.get("language").unwrap(), "rust");
    assert_eq!(vars.get("framework").unwrap(), "none");
    assert_eq!(vars.get("package_manager").unwrap(), "cargo");
    assert_eq!(vars.get("test_framework").unwrap(), "cargo-test");
    assert_eq!(vars.get("linter").unwrap(), "clippy");
    assert_eq!(vars.get("formatter").unwrap(), "rustfmt");
    assert_eq!(vars.get("check_cmd").unwrap(), "cargo check --all-targets");
    assert_eq!(vars.get("test_cmd").unwrap(), "cargo test");
    assert_eq!(vars.get("lint_cmd").unwrap(), "cargo clippy -- -D warnings");
    assert_eq!(vars.get("format_cmd").unwrap(), "cargo fmt");
    assert_eq!(vars.get("format_extensions").unwrap(), "\".rs\"");
    assert_eq!(
        vars.get("companion_pattern").unwrap(),
        "tests/{{name}}_test.rs"
    );
    assert_eq!(vars.get("test_dir").unwrap(), "tests");
    assert_eq!(vars.get("state_prefix").unwrap(), "my-project");
}

#[test]
fn to_template_vars_uses_project_name_for_state_prefix() {
    let preset = sample_preset();
    let vars = preset.to_template_vars("My Cool App");
    assert_eq!(vars.get("state_prefix").unwrap(), "my-cool-app");
}

#[test]
fn export_preset_from_project_dir() {
    let tmp = tempfile::TempDir::new().unwrap();
    let magistr_dir = tmp.path().join(".magistr");
    std::fs::create_dir_all(&magistr_dir).unwrap();

    let config = sample_project_config();
    let json = serde_json::to_string_pretty(&config).unwrap();
    std::fs::write(magistr_dir.join("project.json"), json).unwrap();

    let preset = export_preset(tmp.path()).expect("export");
    assert_eq!(preset.name, "test-project");
    assert_eq!(preset.stack.language, "rust");
}

#[test]
fn export_preset_no_project_json_returns_error() {
    let tmp = tempfile::TempDir::new().unwrap();
    let result = export_preset(tmp.path());
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("project.json"));
}

#[test]
fn sanitize_prefix_works() {
    assert_eq!(sanitize_prefix("My App"), "my-app");
    assert_eq!(sanitize_prefix("hello_world"), "hello-world");
    assert_eq!(sanitize_prefix("already-clean"), "already-clean");
}

#[test]
fn round_trip_quality_gate_conversion() {
    let qg = QualityGate {
        check: "cargo check".into(),
        test: "cargo test".into(),
        lint: "cargo clippy".into(),
    };
    let shareable = ShareableQualityGate::from(&qg);
    let back: QualityGate = QualityGate::from(&shareable);
    assert_eq!(qg, back);
}

#[test]
fn round_trip_format_conversion() {
    let fc = FormatConfig {
        command: "cargo fmt".into(),
        extensions: vec![".rs".into(), ".toml".into()],
        exclusions: vec![],
    };
    let shareable = ShareableFormat::from(&fc);
    let back: FormatConfig = FormatConfig::from(&shareable);
    assert_eq!(fc, back);
}

#[test]
fn round_trip_testing_conversion() {
    let tc = TestingConfig {
        companion_pattern: "tests/{{name}}_test.rs".into(),
        test_dir: "tests".into(),
    };
    let shareable = ShareableTesting::from(&tc);
    let back: TestingConfig = TestingConfig::from(&shareable);
    assert_eq!(tc, back);
}

#[test]
fn description_none_skipped_in_toml() {
    let mut preset = sample_preset();
    preset.description = None;
    let toml_str = preset.to_toml().expect("serialize");
    assert!(!toml_str.contains("description"));
}

// ── Preset validation ───────────────────────────────────────────────

#[test]
fn validate_valid_preset_returns_no_warnings() {
    let preset = sample_preset();
    let warnings = preset.validate();
    assert!(warnings.is_empty(), "expected no warnings: {warnings:?}");
}

#[test]
fn validate_empty_name_returns_error() {
    let mut preset = sample_preset();
    preset.name = String::new();
    let warnings = preset.validate();
    assert!(warnings.iter().any(|w| w.is_error() && w.field == "name"));
}

#[test]
fn validate_empty_language_returns_error() {
    let mut preset = sample_preset();
    preset.stack.language = String::new();
    let warnings = preset.validate();
    assert!(
        warnings
            .iter()
            .any(|w| w.is_error() && w.field == "stack.language")
    );
}

#[test]
fn validate_empty_check_command_returns_warning() {
    let mut preset = sample_preset();
    preset.quality_gate.check = String::new();
    let warnings = preset.validate();
    assert!(warnings.iter().any(|w| w.field == "quality_gate.check"));
}

#[test]
fn validate_empty_test_command_returns_warning() {
    let mut preset = sample_preset();
    preset.quality_gate.test = String::new();
    let warnings = preset.validate();
    assert!(warnings.iter().any(|w| w.field == "quality_gate.test"));
}

#[test]
fn validate_empty_lint_command_returns_warning() {
    let mut preset = sample_preset();
    preset.quality_gate.lint = String::new();
    let warnings = preset.validate();
    assert!(warnings.iter().any(|w| w.field == "quality_gate.lint"));
}

#[test]
fn validate_empty_format_command_returns_warning() {
    let mut preset = sample_preset();
    preset.format.command = String::new();
    let warnings = preset.validate();
    assert!(warnings.iter().any(|w| w.field == "format.command"));
}

#[test]
fn validate_no_format_extensions_returns_warning() {
    let mut preset = sample_preset();
    preset.format.extensions.clear();
    let warnings = preset.validate();
    assert!(warnings.iter().any(|w| w.field == "format.extensions"));
}

#[test]
fn validate_multiple_issues_returns_multiple_warnings() {
    let mut preset = sample_preset();
    preset.name = String::new();
    preset.stack.language = String::new();
    preset.quality_gate.check = String::new();
    let warnings = preset.validate();
    assert!(warnings.len() >= 3);
}

// ── PresetSource parsing ────────────────────────────────────────────

#[test]
fn preset_source_parses_local_path() {
    let source = PresetSource::parse("./my-preset.toml");
    assert!(matches!(source, PresetSource::Local(_)));
}

#[test]
fn preset_source_parses_http_url() {
    let source = PresetSource::parse("http://example.com/preset.toml");
    assert!(matches!(source, PresetSource::Url(_)));
}

#[test]
fn preset_source_parses_https_url() {
    let source = PresetSource::parse("https://example.com/preset.toml");
    assert!(matches!(source, PresetSource::Url(_)));
}

#[test]
fn preset_source_absolute_path_is_local() {
    let source = PresetSource::parse("/tmp/preset.toml");
    assert!(matches!(source, PresetSource::Local(_)));
}

#[test]
fn preset_source_relative_path_is_local() {
    let source = PresetSource::parse("presets/my-team.toml");
    assert!(matches!(source, PresetSource::Local(_)));
}
