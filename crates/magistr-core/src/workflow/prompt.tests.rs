use super::*;
use crate::types::*;

fn sample_config() -> ProjectConfig {
    ProjectConfig {
        project: ProjectMeta {
            name: "test-app".into(),
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
            check: "cargo check".into(),
            test: "cargo test".into(),
            lint: "cargo clippy".into(),
        },
        format: FormatConfig {
            extensions: vec!["rs".into()],
            command: "cargo fmt".into(),
            exclusions: vec![],
        },
        spec: Some(SpecConfig {
            file: "ROADMAP.md".into(),
            unchecked_pattern: r"^\s*- \[ \]".into(),
        }),
        testing: TestingConfig {
            companion_pattern: "tests/{name}.rs".into(),
            test_dir: "tests/".into(),
        },
        reference_repos: vec![],
        state_prefix: "test-app".into(),
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

fn sample_summary() -> SpecSummary {
    SpecSummary {
        total: 10,
        checked: 6,
        unchecked: 4,
        unchecked_items: vec![
            "Item one".into(),
            "Item two".into(),
            "Item three".into(),
            "Item four".into(),
        ],
    }
}

#[test]
fn next_prompt_includes_progress() {
    let prompt = build_next_prompt(&sample_config(), &sample_summary());
    assert!(prompt.contains("6/10 items complete"));
    assert!(prompt.contains("60%"));
    assert!(prompt.contains("4 items"));
}

#[test]
fn next_prompt_includes_unchecked_items() {
    let prompt = build_next_prompt(&sample_config(), &sample_summary());
    assert!(prompt.contains("1. Item one"));
    assert!(prompt.contains("2. Item two"));
    assert!(prompt.contains("4. Item four"));
}

#[test]
fn next_prompt_includes_workflow_reference() {
    let prompt = build_next_prompt(&sample_config(), &sample_summary());
    assert!(prompt.contains("Research-Plan-Execute-Verify"));
    assert!(prompt.contains("magistr_set_phase"));
    assert!(prompt.contains("roadmap_complete_chunk"));
}

#[test]
fn next_prompt_does_not_duplicate_base_workflow() {
    let prompt = build_next_prompt(&sample_config(), &sample_summary());
    assert!(
        !prompt.contains("PHASE 0"),
        "Should not duplicate base workflow phases — those are in the system prompt"
    );
    assert!(
        !prompt.contains("PHASE 1"),
        "Should not duplicate base workflow phases — those are in the system prompt"
    );
}

#[test]
fn next_prompt_includes_project_config() {
    let prompt = build_next_prompt(&sample_config(), &sample_summary());
    assert!(prompt.contains("`cargo check`"));
    assert!(prompt.contains("`cargo test`"));
    assert!(prompt.contains("`cargo clippy`"));
    assert!(prompt.contains("`ROADMAP.md`"));
}

#[test]
fn next_prompt_truncates_many_items() {
    let summary = SpecSummary {
        total: 15,
        checked: 0,
        unchecked: 15,
        unchecked_items: (1..=15).map(|i| format!("Item {i}")).collect(),
    };
    let prompt = build_next_prompt(&sample_config(), &summary);
    assert!(prompt.contains("10. Item 10"));
    assert!(!prompt.contains("11. Item 11"));
    assert!(prompt.contains("... and 5 more items"));
}

#[test]
fn status_line_in_progress() {
    let summary = sample_summary();
    let status = summary.status_line();
    assert!(status.contains("6/10"));
    assert!(status.contains("4 remaining"));
}

#[test]
fn status_line_all_done() {
    let summary = SpecSummary {
        total: 5,
        checked: 5,
        unchecked: 0,
        unchecked_items: vec![],
    };
    let status = summary.status_line();
    assert!(status.contains("complete"));
}

#[test]
fn next_prompt_includes_stop_instruction() {
    let prompt = build_next_prompt(&sample_config(), &sample_summary());
    assert!(prompt.contains("MUST STOP"));
    assert!(prompt.contains("Do NOT invoke `/next`"));
}

#[test]
fn next_prompt_includes_plan_entry_status() {
    let prompt = build_next_prompt(&sample_config(), &sample_summary());
    assert!(prompt.contains("in_progress"));
    assert!(prompt.contains("completed"));
    assert!(prompt.contains("Plan Entry Labeling"));
    assert!(prompt.contains("Plan Entry Status"));
}

/// The shared skill contract owns phase transitions; rule headings are host
/// presentation. Keep the runtime phase parser and both project copies aligned.
#[test]
fn shared_workflow_uses_native_phases_and_matches_project_skills() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let shared =
        std::fs::read_to_string(root.join("crates/magistr-cli/templates/workflows/next.md"))
            .expect("shared next workflow");
    let phases: Vec<_> = shared
        .split("magistr_set_phase(phase: \"")
        .skip(1)
        .map(|call| {
            let name = call.split('"').next().expect("phase argument");
            crate::WorkflowPhase::from_name(name)
                .unwrap_or_else(|| panic!("unknown workflow phase: {name}"))
                .step_index()
        })
        .collect();
    assert_eq!(
        phases,
        (0..=crate::WorkflowPhase::Complete.step_index()).collect::<Vec<_>>(),
        "one complete phase progression in runtime order"
    );
    for tool in ["ship_plan", "ship_start", "ship_complete"] {
        assert!(
            shared.contains(tool),
            "native task lifecycle requires {tool}"
        );
    }
    for (host, invocation) in [(".claude", "/"), (".agents", "$")] {
        let generated = std::fs::read_to_string(root.join(format!("{host}/skills/next/SKILL.md")))
            .expect("project next skill");
        assert_eq!(
            generated,
            shared.replace("{{invoke}}", invocation),
            "{host}"
        );
    }
}
