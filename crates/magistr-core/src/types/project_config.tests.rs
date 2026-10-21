use std::path::PathBuf;

use super::*;
use crate::types::{
    Archetype, ArchetypeRelationship, FormatConfig, ProjectRole, QualityGate, RelationshipKind,
    TestingConfig, Topology,
};

fn sample_config() -> ProjectConfig {
    ProjectConfig {
        verification: Default::default(),
        project: ProjectMeta {
            id: None,
            name: "test-project".to_string(),
            description: "A test project".to_string(),
            project_type: "cli".to_string(),
        },
        stack: StackConfig {
            language: "rust".to_string(),
            framework: "none".to_string(),
            package_manager: "cargo".to_string(),
            test_framework: "cargo-nextest".to_string(),
            linter: "clippy".to_string(),
            formatter: "rustfmt".to_string(),
        },
        quality_gate: QualityGate {
            check: "cargo check".to_string(),
            test: "cargo nextest run".to_string(),
            lint: "cargo clippy -- -D warnings".to_string(),
        },
        format: FormatConfig {
            command: "cargo fmt".to_string(),
            extensions: vec!["rs".to_string()],
            exclusions: vec![],
        },
        spec: Some(SpecConfig {
            file: "ROADMAP.md".to_string(),
            unchecked_pattern: "^\\s*- \\[ \\]".to_string(),
        }),
        testing: TestingConfig {
            companion_pattern: "{name}.tests.{ext}".to_string(),
            test_dir: "alongside".to_string(),
        },
        reference_repos: vec!["https://github.com/example/repo".to_string()],
        state_prefix: "test-project".to_string(),
        custom_gates: vec![],
        guards: Default::default(),
        hints: vec!["Use the magistr MCP server".to_string()],
        lifecycle_hooks: vec![],
        roadmap_backend: Default::default(),
        roadmap_exports: true,
        archetypes: vec![],
        topology: None,
        relationships: vec![],
    }
}

#[test]
fn serde_roundtrip() {
    let config = sample_config();
    let json = serde_json::to_string_pretty(&config).unwrap();
    let back: ProjectConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(config, back);
}

#[test]
fn roadmap_exports_default_true_and_false_survives_config_roundtrip() {
    let mut json = serde_json::to_value(sample_config()).expect("config");
    json.as_object_mut()
        .expect("object")
        .remove("roadmap_exports");
    let mut config: ProjectConfig = serde_json::from_value(json).expect("legacy config");
    assert!(config.roadmap_exports);
    config.roadmap_exports = false;
    let json = serde_json::to_value(&config).expect("config");
    assert_eq!(json["roadmap_exports"], false);
    assert!(
        !serde_json::from_value::<ProjectConfig>(json)
            .expect("roundtrip")
            .roadmap_exports
    );
}

#[test]
fn optional_fields_omitted_when_empty() {
    let config = sample_config();
    let json = serde_json::to_string(&config).unwrap();
    assert!(!json.contains("custom_gates"));
    assert!(!json.contains("lifecycle_hooks"));
}

#[test]
fn deserializes_from_real_project_json() {
    let json = r#"{
        "project": {"name": "foo", "description": "", "type": "cli"},
        "stack": {"language": "python", "framework": "django", "package_manager": "pip", "test_framework": "pytest", "linter": "ruff", "formatter": "black"},
        "quality_gate": {"check": "python -m py_compile .", "test": "pytest", "lint": "ruff check"},
        "format": {"command": "black .", "extensions": ["py"]},
        "spec": {"file": "ROADMAP.md", "unchecked_pattern": "- \\[ \\]"},
        "testing": {"companion_pattern": "test_{name}.py", "test_dir": "tests"},
        "reference_repos": [],
        "state_prefix": "foo"
    }"#;
    let config: ProjectConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.stack.language, "python");
    assert_eq!(config.stack.framework, "django");
    assert!(config.hints.is_empty());
    assert!(config.custom_gates.is_empty());
}

#[test]
fn stack_config_fields() {
    let config = sample_config();
    assert_eq!(config.stack.language, "rust");
    assert_eq!(config.stack.test_framework, "cargo-nextest");
}

#[test]
fn roadmap_backend_defaults_to_workflow_and_is_written() {
    let config = sample_config();
    assert_eq!(config.roadmap_backend, RoadmapBackend::Native);
    let json = serde_json::to_string(&config).expect("serialize");
    assert!(
        json.contains("\"roadmap_backend\":\"native\""),
        "the backend is always written: {json}"
    );
}

#[test]
fn roadmap_backend_yaml_round_trips() {
    let mut config = sample_config();
    config.roadmap_backend = RoadmapBackend::Yaml;
    let json = serde_json::to_string(&config).expect("serialize");
    assert!(json.contains("\"roadmap_backend\":\"yaml\""), "{json}");
    let back: ProjectConfig = serde_json::from_str(&json).expect("deserialize");
    assert!(!back.roadmap_backend.is_native());
}

#[test]
fn deserializes_without_spec_block() {
    // A magistr project carries no YAML spec: `spec` is absent from
    // .magistr/project.json and must not fail deserialization.
    let json = r#"{
        "project": {"name": "foo", "description": "", "type": "cli"},
        "stack": {"language": "rust", "framework": "none", "package_manager": "cargo", "test_framework": "cargo-nextest", "linter": "clippy", "formatter": "rustfmt"},
        "quality_gate": {"check": "cargo check", "test": "cargo nextest run", "lint": "cargo clippy"},
        "format": {"command": "cargo fmt", "extensions": ["rs"]},
        "testing": {"companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside"},
        "reference_repos": [],
        "state_prefix": "foo",
        "roadmap_backend": "native"
    }"#;
    let config: ProjectConfig = serde_json::from_str(json).expect("spec is optional");
    assert!(config.spec.is_none());
    assert!(config.roadmap_backend.is_native());
}

#[test]
fn absent_spec_is_not_written_back() {
    let mut config = sample_config();
    config.spec = None;
    let json = serde_json::to_string(&config).expect("serialize");
    assert!(!json.contains("\"spec\""), "{json}");
    let back: ProjectConfig = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(config, back);
}

// ── Polyglot model tests ─────────────────────────────────────────────

fn make_archetype(id: &str, lang: &str, root: &str) -> Archetype {
    Archetype {
        id: id.into(),
        name: id.into(),
        language: lang.into(),
        framework: None,
        root: PathBuf::from(root),
        role: ProjectRole::Library,
        quality_gate: QualityGate {
            check: format!("{lang} check"),
            test: format!("{lang} test"),
            lint: format!("{lang} lint"),
        },
        format: FormatConfig {
            extensions: vec![],
            command: format!("{lang} fmt"),
            exclusions: vec![],
        },
        testing: TestingConfig {
            companion_pattern: String::new(),
            test_dir: "alongside".into(),
        },
        source_patterns: vec![],
        test_patterns: vec![],
        config_patterns: vec![],
        naming_convention: None,
        module_structure: None,
        model_preference: None,
    }
}

#[test]
fn effective_archetypes_synthesizes_from_legacy() {
    let config = sample_config();
    assert!(config.archetypes.is_empty());
    let effective = config.effective_archetypes();
    assert_eq!(effective.len(), 1);
    assert_eq!(effective[0].id, "default");
    assert_eq!(effective[0].language, "rust");
    assert!(effective[0].framework.is_none()); // "none" → None
    assert_eq!(effective[0].quality_gate.check, "cargo check");
}

#[test]
fn effective_archetypes_returns_explicit_when_present() {
    let mut config = sample_config();
    config.archetypes = vec![
        make_archetype("backend", "rust", "services/api"),
        make_archetype("frontend", "typescript", "apps/web"),
    ];
    let effective = config.effective_archetypes();
    assert_eq!(effective.len(), 2);
    assert_eq!(effective[0].id, "backend");
    assert_eq!(effective[1].id, "frontend");
}

#[test]
fn find_archetype_by_id() {
    let mut config = sample_config();
    config.archetypes = vec![
        make_archetype("api", "rust", "api"),
        make_archetype("web", "typescript", "web"),
    ];
    assert_eq!(config.find_archetype("api").unwrap().language, "rust");
    assert_eq!(config.find_archetype("web").unwrap().language, "typescript");
    assert!(config.find_archetype("missing").is_none());
}

#[test]
fn multi_archetype_project_json_roundtrip() {
    let mut config = sample_config();
    config.archetypes = vec![
        make_archetype("api", "rust", "services/api"),
        make_archetype("web", "typescript", "apps/web"),
    ];
    config.topology = Some(Topology::FlatMonorepo);
    config.relationships = vec![ArchetypeRelationship {
        from: "web".into(),
        to: "api".into(),
        kind: RelationshipKind::DependsOn,
    }];

    let json = serde_json::to_string_pretty(&config).unwrap();
    let back: ProjectConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(back.archetypes.len(), 2);
    assert_eq!(back.topology, Some(Topology::FlatMonorepo));
    assert_eq!(back.relationships.len(), 1);
    assert_eq!(back.relationships[0].kind, RelationshipKind::DependsOn);
}

#[test]
fn omitted_archetypes_use_flat_project_fields() {
    // A project.json with no archetypes/topology/relationships must parse fine.
    let json = r#"{
        "project": {"name": "legacy", "description": "", "type": "cli"},
        "stack": {"language": "go", "framework": "none", "package_manager": "go", "test_framework": "go-test", "linter": "golangci-lint", "formatter": "gofmt"},
        "quality_gate": {"check": "go build ./...", "test": "go test ./...", "lint": "golangci-lint run"},
        "format": {"command": "gofmt -w .", "extensions": ["go"]},
        "testing": {"companion_pattern": "{name}_test.go", "test_dir": "alongside"},
        "reference_repos": [],
        "state_prefix": "legacy"
    }"#;
    let config: ProjectConfig = serde_json::from_str(json).unwrap();
    assert!(config.archetypes.is_empty());
    assert!(config.topology.is_none());
    assert!(config.relationships.is_empty());
    // effective_archetypes synthesizes from flat fields.
    let effective = config.effective_archetypes();
    assert_eq!(effective.len(), 1);
    assert_eq!(effective[0].language, "go");
}

#[test]
fn archetypes_omitted_from_json_when_empty() {
    let config = sample_config();
    let json = serde_json::to_string(&config).unwrap();
    assert!(!json.contains("\"archetypes\""), "{json}");
    assert!(!json.contains("\"topology\""), "{json}");
    assert!(!json.contains("\"relationships\""), "{json}");
}

#[test]
fn framework_none_synthesized_as_absent() {
    let mut config = sample_config();
    config.stack.framework = "none".into();
    let effective = config.effective_archetypes();
    assert!(effective[0].framework.is_none());
}

#[test]
fn framework_value_synthesized_as_some() {
    let mut config = sample_config();
    config.stack.framework = "axum".into();
    let effective = config.effective_archetypes();
    assert_eq!(effective[0].framework.as_deref(), Some("axum"));
}
