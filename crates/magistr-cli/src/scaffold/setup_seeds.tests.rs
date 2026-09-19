use super::*;
use magistr_core::types::Topology;

fn generated() -> String {
    let vars: std::collections::HashMap<String, String> = [
        ("project_name", "test-project"),
        ("project_description", "A test project"),
        ("project_type", "cli"),
        ("language", "rust"),
        ("framework", "none"),
        ("package_manager", "cargo"),
        ("test_framework", "cargo-test"),
        ("linter", "clippy"),
        ("formatter", "rustfmt"),
        ("check_cmd", "cargo check"),
        ("test_cmd", "cargo test"),
        ("lint_cmd", "cargo clippy -- -D warnings"),
        ("format_cmd", "cargo fmt"),
        ("format_extensions", r#""rs""#),
        ("companion_pattern", "tests/{name}.rs"),
        ("test_dir", "tests/"),
        ("state_prefix", "test-project"),
        ("reference_repos", ""),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
    crate::scaffold::substitute(crate::scaffold::TMPL_PROJECT_JSON, &vars)
}

#[test]
fn nothing_to_plant_keeps_the_template_bytes() {
    let template = generated();
    assert_eq!(SetupSeeds::default().project_json(&template), template);
    assert_eq!(SetupSeeds::default().claude_md("# Doc\n"), "# Doc\n");
}

#[test]
fn hooks_are_planted_in_the_configuration() {
    let seeds = SetupSeeds {
        lifecycle_hooks: magistr_engine::workflow::hook_presets::preset_for_language("rust"),
        archetypes: None,
    };
    let planted = seeds.project_json(&generated());
    let config: crate::types::ProjectConfig = serde_json::from_str(&planted).unwrap();
    assert_eq!(config.lifecycle_hooks, seeds.lifecycle_hooks);
    assert_eq!(config.quality_gate.check, "cargo check");
    assert!(planted.ends_with("}\n"));
}

#[test]
fn a_polyglot_tree_is_planted_in_the_configuration_and_the_document() {
    let polyglot = TreeToConfigResult {
        archetypes: vec![],
        topology: Topology::Workspace,
        relationships: vec![],
    };
    let seeds = SetupSeeds {
        lifecycle_hooks: vec![],
        archetypes: Some(polyglot),
    };
    let config: crate::types::ProjectConfig =
        serde_json::from_str(&seeds.project_json(&generated())).unwrap();
    assert_eq!(config.topology, Some(Topology::Workspace));
    // No archetypes: the document keeps its bytes.
    assert_eq!(seeds.claude_md("# Doc\n"), "# Doc\n");
}

#[test]
fn an_unreadable_configuration_keeps_the_authors_bytes() {
    let seeds = SetupSeeds {
        lifecycle_hooks: magistr_engine::workflow::hook_presets::preset_for_language("rust"),
        archetypes: None,
    };
    assert_eq!(seeds.project_json("{ not json"), "{ not json");
}
