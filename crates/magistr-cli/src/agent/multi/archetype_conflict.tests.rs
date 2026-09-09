use std::path::PathBuf;

use magistr_core::types::{ArchetypeRelationship, RelationshipKind};

use super::*;

#[test]
fn no_modified_files_no_conflicts() {
    let conflicts = detect_cross_archetype_conflicts(&[], &[]);
    assert!(conflicts.is_empty());
}

#[test]
fn non_contract_file_no_conflict() {
    let modified = vec![(
        AgentId::new(1),
        "backend".into(),
        PathBuf::from("src/main.rs"),
    )];
    let rels = vec![ArchetypeRelationship {
        from: "frontend".into(),
        to: "backend".into(),
        kind: RelationshipKind::DependsOn,
    }];
    let conflicts = detect_cross_archetype_conflicts(&modified, &rels);
    assert!(conflicts.is_empty());
}

#[test]
fn api_contract_change_detected() {
    let modified = vec![(
        AgentId::new(1),
        "backend".into(),
        PathBuf::from("api/openapi.json"),
    )];
    let rels = vec![ArchetypeRelationship {
        from: "frontend".into(),
        to: "backend".into(),
        kind: RelationshipKind::DependsOn,
    }];
    let conflicts = detect_cross_archetype_conflicts(&modified, &rels);
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].kind, ConflictKind::ApiContractChanged);
    assert!(conflicts[0].archetype_ids.contains(&"backend".into()));
    assert!(conflicts[0].archetype_ids.contains(&"frontend".into()));
}

#[test]
fn shared_type_modification_detected() {
    let modified = vec![(
        AgentId::new(2),
        "shared".into(),
        PathBuf::from("shared/types.ts"),
    )];
    let rels = vec![ArchetypeRelationship {
        from: "shared".into(),
        to: "frontend".into(),
        kind: RelationshipKind::SharesWith,
    }];
    let conflicts = detect_cross_archetype_conflicts(&modified, &rels);
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].kind, ConflictKind::SharedTypeModified);
}

#[test]
fn dependency_version_conflict_detected() {
    let modified = vec![(
        AgentId::new(1),
        "backend".into(),
        PathBuf::from("backend/Cargo.toml"),
    )];
    let rels = vec![ArchetypeRelationship {
        from: "frontend".into(),
        to: "backend".into(),
        kind: RelationshipKind::DependsOn,
    }];
    let conflicts = detect_cross_archetype_conflicts(&modified, &rels);
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].kind, ConflictKind::DependencyVersionConflict);
}

#[test]
fn no_relationship_no_conflict() {
    let modified = vec![(
        AgentId::new(1),
        "backend".into(),
        PathBuf::from("api/openapi.json"),
    )];
    // No relationships at all.
    let conflicts = detect_cross_archetype_conflicts(&modified, &[]);
    assert!(conflicts.is_empty());
}

#[test]
fn is_in_conflict_zone_detects_contract_files() {
    use std::path::Path;
    assert!(is_in_conflict_zone(Path::new("openapi.json"), &[]));
    assert!(is_in_conflict_zone(Path::new("schema.graphql"), &[]));
    assert!(is_in_conflict_zone(Path::new("types.ts"), &[]));
    assert!(is_in_conflict_zone(Path::new("package.json"), &[]));
    assert!(!is_in_conflict_zone(Path::new("main.rs"), &[]));
    assert!(!is_in_conflict_zone(Path::new("index.tsx"), &[]));
}
