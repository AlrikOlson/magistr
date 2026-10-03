use super::*;

#[test]
fn task_evidence_retains_recent_records_and_bounds_unicode_details() {
    let checks: Vec<Value> = (0..85)
        .map(|n| {
            json!({
                "type": "test", "name": format!("check {n}"), "passed": true,
                "details": "é".repeat(7000), "required": true, "verified": true,
                "timestamp": "2026-10-02T00:00:00Z"
            })
        })
        .collect();
    let actions: Vec<Value> = (0..83)
        .map(|n| {
            json!({
                "id": n, "task_id": "evidence", "timestamp": "2026-10-02T00:00:00Z",
                "type": "test", "description": format!("action {n}"),
                "files_touched": [], "tools_used": [], "result": ""
            })
        })
        .collect();
    let task = serde_json::from_value(json!({
        "id": "evidence", "title": "Inspect evidence", "type": "test",
        "status": "active", "estimate": null, "checks": checks, "actions": actions
    }))
    .expect("canonical task");
    let view = bounded_task(task);
    assert_eq!(view["omitted_checks"], 5);
    assert_eq!(view["omitted_actions"], 3);
    assert_eq!(view["record"]["checks"].as_array().unwrap().len(), 80);
    assert_eq!(view["record"]["checks"][0]["name"], "check 5");
    assert_eq!(
        view["record"]["checks"][0]["details"]
            .as_str()
            .unwrap()
            .chars()
            .count(),
        6000
    );
    assert_eq!(view["record"]["actions"][0]["id"], 3);
}

#[test]
fn query_options_refuse_unknown_malformed_empty_and_oversized_selectors() {
    assert!(validate_args(&json!({})).is_ok());
    assert!(validate_args(&json!({"task_id": "task", "receipt_id": "receipt"})).is_ok());
    for args in [
        json!(null),
        json!({"task_id": 3}),
        json!({"receipt_id": ""}),
        json!({"task_id": "x".repeat(257)}),
        json!({"run": true}),
    ] {
        assert!(validate_args(&args).is_err(), "{args}");
    }
}

#[test]
fn phase_is_unknown_until_announced_and_scoped_to_cycle_and_task() {
    let state = ContextState::default();
    let cycle = Some("cycle".to_owned());
    let task = Some("task".to_owned());
    assert_eq!(state.phase(&cycle, &task), None);
    state.observe(cycle.clone(), task.clone(), WorkflowPhase::Verifying);
    assert_eq!(state.phase(&cycle, &task), Some(WorkflowPhase::Verifying));
    assert_eq!(state.phase(&Some("new cycle".to_owned()), &task), None);
    assert_eq!(state.phase(&cycle, &None), None);
    assert_eq!(ContextState::default().phase(&cycle, &task), None);
}

#[test]
fn check_choices_use_the_existing_resolver_and_include_optional_checks() {
    let config: ProjectConfig = serde_json::from_value(json!({
        "project": {"name": "context", "type": "cli"},
        "stack": {"language": "rust", "framework": "none", "package_manager": "cargo",
            "test_framework": "cargo-test", "linter": "clippy", "formatter": "rustfmt"},
        "format": {"command": "cargo fmt", "extensions": ["rs"]},
        "testing": {"companion_pattern": "{name}.tests.rs", "test_dir": "alongside"},
        "reference_repos": [], "state_prefix": "context",
        "quality_gate": {"check": "cargo check", "test": "", "lint": "cargo clippy"},
        "custom_gates": [{"name": "contrast", "command": "contrast-check", "required": false}]
    }))
    .expect("project config");
    let choices = check_choices(&config).expect("resolved");
    let contrast = choices
        .iter()
        .find(|c| c["key"] == "contrast")
        .expect("custom");
    assert_eq!(contrast["commands"][0]["command"], "contrast-check");
    assert_eq!(contrast["commands"][0]["required"], false);
    let test = choices.iter().find(|c| c["key"] == "test").expect("test");
    assert_eq!(test["commands"], json!([]));
    assert!(!choices.iter().any(|c| c["key"] == "all"));
}

#[test]
fn descriptor_is_a_read_query_with_no_required_arguments() {
    let query = tool();
    assert_eq!(query.name, "magistr_context");
    assert_eq!(query.input_schema["additionalProperties"], false);
}
