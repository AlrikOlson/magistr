use super::*;

fn sample_config() -> PromptConfig {
    PromptConfig {
        spec_file: Some("ROADMAP.md".into()),
        check_command: "cargo check".into(),
        test_command: "cargo test".into(),
        lint_command: "cargo clippy".into(),
        format_command: "cargo fmt".into(),
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
    let prompt = sample_config().build_next_prompt(&sample_summary());
    assert!(prompt.contains("6/10 items complete"));
    assert!(prompt.contains("60%"));
    assert!(prompt.contains("4 items"));
}

#[test]
fn next_prompt_includes_unchecked_items() {
    let prompt = sample_config().build_next_prompt(&sample_summary());
    assert!(prompt.contains("1. Item one"));
    assert!(prompt.contains("2. Item two"));
    assert!(prompt.contains("4. Item four"));
}

#[test]
fn next_prompt_includes_workflow_reference() {
    let prompt = sample_config().build_next_prompt(&sample_summary());
    assert!(prompt.contains("Research-Plan-Execute-Verify"));
    assert!(prompt.contains("magistr_set_phase"));
    assert!(prompt.contains("roadmap_complete_chunk"));
    assert!(prompt.contains("ship_finalize"));
    assert!(!prompt.contains("magistr_roadmap"));
}

#[test]
fn native_and_file_iteration_prompts_carry_the_shared_recording_contract() {
    for spec_file in [None, Some("plan.md".to_owned())] {
        let config = PromptConfig {
            spec_file,
            ..sample_config()
        };
        let prompt = config.build_next_prompt(&sample_summary());
        assert!(!PromptConfig::REASONING_GUIDANCE.is_empty());
        assert_eq!(prompt.matches(PromptConfig::REASONING_GUIDANCE).count(), 1);
    }
}

#[test]
fn selected_native_work_keeps_its_resume_marker_and_recovery_contract() {
    let config = PromptConfig {
        spec_file: None,
        ..sample_config()
    };
    for selected in [
        "retry-recovery — Restore interrupted delivery (in progress; resume this work)",
        "retry-recovery — Restore interrupted delivery",
    ] {
        let summary = SpecSummary {
            total: 12,
            checked: 8,
            unchecked: 4,
            unchecked_items: vec![selected.into()],
        };
        let prompt = config.build_next_prompt(&summary);
        let selected_section = format!("### Selected Work\n\n1. {selected}\n");
        assert!(prompt.contains(&selected_section), "{prompt}");
        assert_eq!(prompt.matches(selected).count(), 1);
        assert!(!prompt.contains("### Next Unchecked Items"));
        assert_eq!(
            prompt.matches(PromptConfig::NEXT_RECOVERY_GUIDANCE).count(),
            1
        );
        assert!(prompt.find(&selected_section) < prompt.find(PromptConfig::NEXT_RECOVERY_GUIDANCE));
        assert!(
            prompt.find(PromptConfig::NEXT_RECOVERY_GUIDANCE)
                < prompt.find(NEXT_WORKFLOW_INSTRUCTIONS)
        );
        assert!(prompt.contains("MUST STOP"));
    }
}

#[test]
fn file_iteration_recovery_preserves_the_existing_item_preview() {
    let prompt = sample_config().build_next_prompt(&sample_summary());
    assert!(prompt.contains("### Next Unchecked Items\n\n1. Item one"));
    assert!(!prompt.contains("### Selected Work"));
    assert_eq!(
        prompt.matches(PromptConfig::NEXT_RECOVERY_GUIDANCE).count(),
        1
    );
}

#[test]
fn next_prompt_does_not_duplicate_base_workflow() {
    let prompt = sample_config().build_next_prompt(&sample_summary());
    assert!(
        !prompt.contains("PHASE 0"),
        "Should not duplicate base workflow phases"
    );
    assert!(
        !prompt.contains("PHASE 1"),
        "Should not duplicate base workflow phases"
    );
}

#[test]
fn next_prompt_includes_project_config() {
    let prompt = sample_config().build_next_prompt(&sample_summary());
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
    let prompt = sample_config().build_next_prompt(&summary);
    assert!(prompt.contains("10. Item 10"));
    assert!(!prompt.contains("11. Item 11"));
    assert!(prompt.contains("... and 5 more items"));
}

#[test]
fn next_prompt_includes_stop_instruction() {
    let prompt = sample_config().build_next_prompt(&sample_summary());
    assert!(prompt.contains("MUST STOP"));
    assert!(prompt.contains("Do NOT invoke `/next`"));
}

#[test]
fn next_prompt_includes_plan_entry_status() {
    let prompt = sample_config().build_next_prompt(&sample_summary());
    assert!(prompt.contains("in_progress"));
    assert!(prompt.contains("completed"));
    assert!(prompt.contains("Plan Entry Labeling"));
    assert!(prompt.contains("Plan Entry Status"));
}

#[test]
fn next_prompt_points_at_workflow_without_a_spec_file() {
    let config = PromptConfig {
        spec_file: None,
        ..sample_config()
    };
    let prompt = config.build_next_prompt(&sample_summary());
    assert!(
        prompt.contains("`roadmap_next`"),
        "no spec file — the agent must be sent to the engines: {prompt}"
    );
    assert!(
        !prompt.contains("**Roadmap:** `"),
        "must not invent a spec file to read: {prompt}"
    );
}
