use std::path::{Path, PathBuf};

use super::*;

fn make_archetype(id: &str, root: &str, lang: &str) -> Archetype {
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
            command: String::new(),
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
fn resolve_single_archetype_always_matches() {
    let archetypes = vec![make_archetype("main", ".", "rust")];
    let result = resolve_archetype(&archetypes, Path::new("src/lib.rs"));
    assert_eq!(result.unwrap().id, "main");
}

#[test]
fn resolve_empty_returns_none() {
    let result = resolve_archetype(&[], Path::new("src/lib.rs"));
    assert!(result.is_none());
}

#[test]
fn resolve_longest_prefix_wins() {
    let archetypes = vec![
        make_archetype("root", ".", "rust"),
        make_archetype("web", "apps/web", "typescript"),
        make_archetype("api", "apps/api", "rust"),
    ];

    assert_eq!(
        resolve_archetype(&archetypes, Path::new("apps/web/src/index.ts"))
            .unwrap()
            .id,
        "web"
    );
    assert_eq!(
        resolve_archetype(&archetypes, Path::new("apps/api/src/main.rs"))
            .unwrap()
            .id,
        "api"
    );
    // Falls back to root for unmatched paths.
    assert_eq!(
        resolve_archetype(&archetypes, Path::new("README.md"))
            .unwrap()
            .id,
        "root"
    );
}

#[test]
fn relationship_serde_roundtrip() {
    let rel = ArchetypeRelationship {
        from: "web".into(),
        to: "shared".into(),
        kind: RelationshipKind::DependsOn,
    };
    let json = serde_json::to_string(&rel).unwrap();
    let parsed: ArchetypeRelationship = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, rel);
    assert!(json.contains("depends-on"));
}

#[test]
fn archetype_serde_roundtrip() {
    let arch = make_archetype("backend", "services/api", "rust");
    let json = serde_json::to_string_pretty(&arch).unwrap();
    let parsed: Archetype = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, arch);
}
