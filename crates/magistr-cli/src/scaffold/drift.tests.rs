use super::*;

use magistr_core::types::{
    Archetype, DetectedProject, FormatConfig, Language, ProjectMetadata, ProjectRole, ProjectTree,
    QualityGate, TestingConfig, Topology, Workspace,
};
use std::path::PathBuf;

fn make_archetype(id: &str, language: &str, root: &str) -> Archetype {
    Archetype {
        id: id.to_string(),
        name: id.to_string(),
        language: language.to_string(),
        framework: None,
        root: PathBuf::from(root),
        role: ProjectRole::Backend,
        quality_gate: QualityGate {
            check: "cargo check".into(),
            test: "cargo test".into(),
            lint: "cargo clippy".into(),
        },
        format: FormatConfig {
            extensions: vec![],
            command: String::new(),
            exclusions: vec![],
        },
        testing: TestingConfig {
            companion_pattern: String::new(),
            test_dir: String::new(),
        },
        source_patterns: vec![],
        test_patterns: vec![],
        config_patterns: vec![],
        naming_convention: None,
        module_structure: None,
        model_preference: None,
    }
}

fn make_detected(path: &str, language: Language, name: &str) -> DetectedProject {
    DetectedProject {
        path: PathBuf::from(path),
        language,
        framework: None,
        role: ProjectRole::Backend,
        markers: vec![],
        metadata: ProjectMetadata {
            name: Some(name.to_string()),
            dependencies: vec!["tokio".to_string()],
            version: None,
        },
        parent: None,
        children: vec![],
    }
}

#[test]
fn no_drift_when_matching() {
    let archetypes = vec![make_archetype("api", "rust", "services/api")];
    let tree = ProjectTree {
        root: PathBuf::from("/tmp"),
        projects: vec![make_detected("services/api", Language::Rust, "api")],
        edges: vec![],
        workspace: Workspace::None,
        topology: Topology::SingleProject,
    };

    let report = detect_drift(&archetypes, &tree);
    assert!(report.is_clean(), "report should be clean: {:?}", report);
}

#[test]
fn detects_new_project() {
    let archetypes = vec![make_archetype("api", "rust", "services/api")];
    let tree = ProjectTree {
        root: PathBuf::from("/tmp"),
        projects: vec![
            make_detected("services/api", Language::Rust, "api"),
            make_detected("apps/web", Language::TypeScript, "web"),
        ],
        edges: vec![],
        workspace: Workspace::None,
        topology: Topology::FlatMonorepo,
    };

    let report = detect_drift(&archetypes, &tree);
    assert!(!report.is_clean());
    assert_eq!(report.new_projects.len(), 1);
    assert_eq!(report.new_projects[0].path, PathBuf::from("apps/web"));
}

#[test]
fn detects_removed_project() {
    let archetypes = vec![
        make_archetype("api", "rust", "services/api"),
        make_archetype("web", "typescript", "apps/web"),
    ];
    let tree = ProjectTree {
        root: PathBuf::from("/tmp"),
        projects: vec![make_detected("services/api", Language::Rust, "api")],
        edges: vec![],
        workspace: Workspace::None,
        topology: Topology::SingleProject,
    };

    let report = detect_drift(&archetypes, &tree);
    assert!(!report.is_clean());
    assert_eq!(report.removed_archetypes.len(), 1);
    assert_eq!(report.removed_archetypes[0], "web");
}

#[test]
fn detects_language_change() {
    let archetypes = vec![make_archetype("api", "python", "services/api")];
    let tree = ProjectTree {
        root: PathBuf::from("/tmp"),
        projects: vec![make_detected("services/api", Language::Rust, "api")],
        edges: vec![],
        workspace: Workspace::None,
        topology: Topology::SingleProject,
    };

    let report = detect_drift(&archetypes, &tree);
    assert!(!report.is_clean());
    assert_eq!(report.changes.len(), 1);
    assert!(report.changes[0].description.contains("python"));
    assert!(report.changes[0].description.contains("rust"));
}
