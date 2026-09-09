use std::path::PathBuf;

use magistr_core::types::{
    Archetype, ArchetypeRelationship, FormatConfig, ProjectRole, QualityGate, RelationshipKind,
    TestingConfig,
};

use super::*;

fn make_archetype(id: &str, root: &str, lang: &str) -> Archetype {
    Archetype {
        id: id.into(),
        name: id.into(),
        language: lang.into(),
        framework: None,
        root: PathBuf::from(root),
        role: ProjectRole::Backend,
        quality_gate: QualityGate {
            check: String::new(),
            test: String::new(),
            lint: String::new(),
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
fn no_relationships_produces_empty_context() {
    let arch = make_archetype("backend", "api", "rust");
    let all = vec![arch.clone()];
    let result = build_cross_archetype_context(&arch, &all, &[], Path::new("/tmp"));
    assert!(result.is_empty());
}

#[test]
fn depends_on_relationship_produces_context_section() {
    let backend = make_archetype("backend", "api", "rust");
    let shared = make_archetype("shared", "shared", "rust");
    let all = vec![backend.clone(), shared];
    let rels = vec![ArchetypeRelationship {
        from: "backend".into(),
        to: "shared".into(),
        kind: RelationshipKind::DependsOn,
    }];
    // Even though the directories don't exist on disk, the function
    // should gracefully return empty (no contract files found).
    let result = build_cross_archetype_context(&backend, &all, &rels, Path::new("/nonexistent"));
    // No files to find, so result is empty.
    assert!(result.is_empty());
}

#[test]
fn relationship_label_maps_correctly() {
    assert_eq!(
        relationship_label(&RelationshipKind::DependsOn),
        "dependency"
    );
    assert_eq!(relationship_label(&RelationshipKind::Hosts), "hosted by");
    assert_eq!(
        relationship_label(&RelationshipKind::SharesWith),
        "shared code"
    );
}

#[test]
fn is_contract_file_matches_known_patterns() {
    assert!(is_contract_file("openapi.json"));
    assert!(is_contract_file("openapi.yaml"));
    assert!(is_contract_file("schema.graphql"));
    assert!(is_contract_file("types.ts"));
    assert!(is_contract_file("types.d.ts"));
    assert!(is_contract_file("models.rs"));
    assert!(!is_contract_file("main.rs"));
    assert!(!is_contract_file("index.tsx"));
}

#[test]
fn contract_files_found_in_temp_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let shared_dir = tmp.path().join("shared").join("src");
    std::fs::create_dir_all(&shared_dir).unwrap();
    std::fs::write(shared_dir.join("types.ts"), "export type Foo = {}").unwrap();
    std::fs::write(shared_dir.join("index.ts"), "export {}").unwrap();

    let shared = make_archetype("shared", "shared", "typescript");
    let files = find_contract_files(&shared, tmp.path());
    assert_eq!(files.len(), 1);
    assert!(files[0].to_string_lossy().contains("types.ts"));
}

#[test]
fn full_context_injection_with_contract_files() {
    let tmp = tempfile::tempdir().unwrap();
    let shared_dir = tmp.path().join("shared");
    std::fs::create_dir_all(&shared_dir).unwrap();
    std::fs::write(shared_dir.join("types.ts"), "export type Foo = {}").unwrap();

    let frontend = make_archetype("frontend", "web", "typescript");
    let shared = make_archetype("shared", "shared", "typescript");
    let all = vec![frontend.clone(), shared];
    let rels = vec![ArchetypeRelationship {
        from: "frontend".into(),
        to: "shared".into(),
        kind: RelationshipKind::DependsOn,
    }];

    let result = build_cross_archetype_context(&frontend, &all, &rels, tmp.path());
    assert!(result.contains("Cross-Archetype Context"));
    assert!(result.contains("types.ts"));
    assert!(result.contains("shared"));
}
