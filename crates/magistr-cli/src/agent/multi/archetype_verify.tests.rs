use std::path::PathBuf;

use magistr_core::types::{
    Archetype, ArchetypeRelationship, FormatConfig, ProjectRole, QualityGate, RelationshipKind,
    TestingConfig,
};

use super::*;

fn make_archetype(id: &str) -> Archetype {
    Archetype {
        id: id.into(),
        name: id.into(),
        language: "rust".into(),
        framework: None,
        root: PathBuf::from(id),
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
fn empty_archetypes_gives_empty_order() {
    let order = gate_execution_order(&[], &[]);
    assert!(order.is_empty());
}

#[test]
fn single_archetype_returns_itself() {
    let archs = vec![make_archetype("backend")];
    let order = gate_execution_order(&archs, &[]);
    assert_eq!(order, vec!["backend"]);
}

#[test]
fn no_deps_preserves_sorted_order() {
    let archs = vec![
        make_archetype("frontend"),
        make_archetype("backend"),
        make_archetype("shared"),
    ];
    let order = gate_execution_order(&archs, &[]);
    // Kahn's with sorting gives alphabetical.
    assert_eq!(order, vec!["backend", "frontend", "shared"]);
}

#[test]
fn depends_on_orders_dependency_first() {
    let archs = vec![make_archetype("frontend"), make_archetype("backend")];
    let rels = vec![ArchetypeRelationship {
        from: "frontend".into(),
        to: "backend".into(),
        kind: RelationshipKind::DependsOn,
    }];
    let order = gate_execution_order(&archs, &rels);
    assert_eq!(order, vec!["backend", "frontend"]);
}

#[test]
fn diamond_dependency_order() {
    let archs = vec![
        make_archetype("app"),
        make_archetype("backend"),
        make_archetype("frontend"),
        make_archetype("shared"),
    ];
    let rels = vec![
        ArchetypeRelationship {
            from: "backend".into(),
            to: "shared".into(),
            kind: RelationshipKind::DependsOn,
        },
        ArchetypeRelationship {
            from: "frontend".into(),
            to: "shared".into(),
            kind: RelationshipKind::DependsOn,
        },
        ArchetypeRelationship {
            from: "app".into(),
            to: "backend".into(),
            kind: RelationshipKind::DependsOn,
        },
        ArchetypeRelationship {
            from: "app".into(),
            to: "frontend".into(),
            kind: RelationshipKind::DependsOn,
        },
    ];
    let order = gate_execution_order(&archs, &rels);
    // shared must be first, app must be last.
    assert_eq!(order[0], "shared");
    assert_eq!(*order.last().unwrap(), "app");
}

#[test]
fn shares_with_does_not_affect_order() {
    let archs = vec![make_archetype("a"), make_archetype("b")];
    let rels = vec![ArchetypeRelationship {
        from: "a".into(),
        to: "b".into(),
        kind: RelationshipKind::SharesWith,
    }];
    let order = gate_execution_order(&archs, &rels);
    // No DependsOn edges, so alphabetical.
    assert_eq!(order, vec!["a", "b"]);
}

#[test]
fn aggregate_all_pass() {
    let results = vec![
        ArchetypeGateResult {
            archetype_id: "backend".into(),
            passed: true,
            gates: vec![],
        },
        ArchetypeGateResult {
            archetype_id: "frontend".into(),
            passed: true,
            gates: vec![],
        },
    ];
    assert!(aggregate_results(&results));
}

#[test]
fn aggregate_one_fails() {
    let results = vec![
        ArchetypeGateResult {
            archetype_id: "backend".into(),
            passed: true,
            gates: vec![],
        },
        ArchetypeGateResult {
            archetype_id: "frontend".into(),
            passed: false,
            gates: vec![GateOutcome {
                name: "lint".into(),
                passed: false,
                exit_code: Some(1),
            }],
        },
    ];
    assert!(!aggregate_results(&results));
}
