use super::*;
use crate::commands::run::HeadlessOutcome;
use crate::workflow::spec::SpecSummary;
use std::time::Duration;

fn make_summary(checked: usize, total: usize) -> SpecSummary {
    SpecSummary {
        total,
        checked,
        unchecked: total - checked,
        unchecked_items: vec![],
    }
}

#[test]
fn workflow_session_uses_selected_project_for_execution_and_context() {
    let current_dir = std::env::current_dir().unwrap();
    let project = tempfile::tempdir().unwrap();
    std::fs::create_dir(project.path().join(".magistr")).unwrap();
    std::fs::create_dir_all(project.path().join(".claude/rules")).unwrap();
    let config = serde_json::json!({
        "project": {"name":"selected-project"},
        "stack": {"language":"rust", "framework":"none", "package_manager":"cargo",
            "test_framework":"cargo-test", "linter":"clippy", "formatter":"rustfmt"},
        "quality_gate": {"check":"selected-check", "test":"selected-test", "lint":"selected-lint"},
        "custom_gates":[{"name":"selected-custom","command":"selected-custom-command","required":true}],
        "format": {"command":"true", "extensions":[]},
        "testing": {"companion_pattern":"{name}.tests.{ext}", "test_dir":"alongside"},
        "reference_repos":[], "state_prefix":"selected-project"
    });
    std::fs::write(
        project.path().join(".magistr/project.json"),
        serde_json::to_vec(&config).unwrap(),
    )
    .unwrap();
    std::fs::write(
        project.path().join(".claude/rules/local.md"),
        "Selected project rule sentinel",
    )
    .unwrap();

    let session = workflow_agent_config("Do the next item", project.path(), agent::Backend::Acp);
    assert_eq!(session.cwd, project.path());
    assert!(session.workflow_permissions);
    assert_eq!(
        crate::commands::run::collect_gate_commands(project.path()),
        vec![
            "selected-check",
            "selected-test",
            "selected-lint",
            "selected-custom-command"
        ]
    );
    let prompt = session.system_prompt.unwrap();
    assert!(prompt.contains("**Project:** selected-project"));
    assert!(prompt.contains("Selected project rule sentinel"));
    assert_eq!(std::env::current_dir().unwrap(), current_dir);
}

#[test]
fn emit_report_json_success() {
    let before = make_summary(107, 204);
    let after = make_summary(110, 204);
    let duration = Duration::from_secs_f64(180.5);

    let mut buf = Vec::new();
    emit_report(
        &HeadlessFormat::Json,
        &HeadlessOutcome::Ok,
        &before,
        &after,
        duration,
        &mut buf,
    )
    .unwrap();

    let output: serde_json::Value = serde_json::from_slice(&buf).unwrap();
    assert_eq!(output["type"], "iteration_report");
    assert_eq!(output["status"], "success");
    assert_eq!(output["spec_before"]["checked"], 107);
    assert_eq!(output["spec_before"]["total"], 204);
    assert_eq!(output["spec_after"]["checked"], 110);
    assert_eq!(output["spec_after"]["total"], 204);
    assert!(output["duration_secs"].as_f64().unwrap() > 180.0);
}

#[test]
fn emit_report_json_agent_error() {
    let summary = make_summary(50, 100);
    let duration = Duration::from_secs(60);

    let mut buf = Vec::new();
    emit_report(
        &HeadlessFormat::Json,
        &HeadlessOutcome::AgentError,
        &summary,
        &summary,
        duration,
        &mut buf,
    )
    .unwrap();

    let output: serde_json::Value = serde_json::from_slice(&buf).unwrap();
    assert_eq!(output["status"], "agent_error");
}

#[test]
fn emit_report_json_budget_exceeded() {
    let summary = make_summary(50, 100);
    let duration = Duration::from_secs(30);

    let mut buf = Vec::new();
    emit_report(
        &HeadlessFormat::Json,
        &HeadlessOutcome::BudgetExceeded,
        &summary,
        &summary,
        duration,
        &mut buf,
    )
    .unwrap();

    let output: serde_json::Value = serde_json::from_slice(&buf).unwrap();
    assert_eq!(output["status"], "budget_exceeded");
}

#[test]
fn emit_report_plain_shows_spec_progress() {
    let before = make_summary(10, 50);
    let after = make_summary(15, 50);
    let duration = Duration::from_secs(90);

    let mut buf = Vec::new();
    emit_report(
        &HeadlessFormat::Plain,
        &HeadlessOutcome::Ok,
        &before,
        &after,
        duration,
        &mut buf,
    )
    .unwrap();

    let output = String::from_utf8(buf).unwrap();
    assert!(output.contains("10/50 -> 15/50"));
    assert!(output.contains("success"));
    assert!(output.contains("90.0s"));
}
