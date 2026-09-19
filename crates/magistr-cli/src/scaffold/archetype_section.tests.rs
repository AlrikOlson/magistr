use super::*;
use magistr_core::types::{FormatConfig, ProjectRole, QualityGate, TestingConfig};

fn archetype(id: &str, root: &str) -> Archetype {
    Archetype {
        id: id.to_owned(),
        name: id.to_owned(),
        language: "rust".to_owned(),
        framework: None,
        root: root.into(),
        role: ProjectRole::Backend,
        quality_gate: QualityGate {
            check: "cargo check".to_owned(),
            test: "cargo test".to_owned(),
            lint: "cargo clippy".to_owned(),
        },
        format: FormatConfig {
            extensions: vec!["rs".to_owned()],
            command: "cargo fmt".to_owned(),
            exclusions: Vec::new(),
        },
        testing: TestingConfig {
            companion_pattern: "{name}.tests.rs".to_owned(),
            test_dir: "alongside".to_owned(),
        },
        source_patterns: Vec::new(),
        test_patterns: Vec::new(),
        config_patterns: Vec::new(),
        naming_convention: None,
        module_structure: None,
        model_preference: None,
    }
}

#[test]
fn appends_the_section_to_a_document_without_markers() {
    let doc = "# Hello\n\nSome content.\n";
    let result = with_archetype_section(
        doc,
        &[archetype("api", "services/api")],
        Topology::FlatMonorepo,
    );
    assert!(result.starts_with("# Hello\n\nSome content.\n\n<!-- archetypes -->\n"));
    assert!(result.contains("**Topology:** Flat Monorepo"));
    assert!(result.contains(
        "| api | rust | `services/api` | `cargo check` | `cargo test` | `cargo clippy` |"
    ));
    assert!(result.ends_with("<!-- /archetypes -->\n"));
}

#[test]
fn refreshes_the_section_between_existing_markers() {
    let doc = "Before\n<!-- archetypes -->\nOld stuff\n<!-- /archetypes -->\nAfter\n";
    let result = with_archetype_section(doc, &[archetype("web", "apps/web")], Topology::Workspace);
    assert!(result.starts_with("Before\n<!-- archetypes -->\n## Project Archetypes"));
    assert!(result.ends_with("<!-- /archetypes -->\nAfter\n"));
    assert!(result.contains("| web |"));
    assert!(!result.contains("Old stuff"));
}

#[test]
fn a_matching_section_is_a_fixed_point() {
    let once = with_archetype_section(
        "# Doc\n",
        &[archetype("api", "api")],
        Topology::SingleProject,
    );
    let twice = with_archetype_section(&once, &[archetype("api", "api")], Topology::SingleProject);
    assert_eq!(once, twice);
}
