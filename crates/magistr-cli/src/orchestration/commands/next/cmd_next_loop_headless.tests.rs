use super::*;
use crate::agent::message::{AgentMessage, SessionId};
use crate::agent::mock::mock_session;
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

fn result_message(cost: f64) -> AgentMessage {
    AgentMessage::Result {
        subtype: "result".into(),
        duration_ms: 100,
        duration_api_ms: 80,
        is_error: false,
        num_turns: 1,
        session_id: SessionId::new("loop-budget-test"),
        total_cost_usd: Some(cost),
        usage: None,
        result: None,
    }
}

#[tokio::test]
async fn loop_budget_uses_recorded_spend_across_sessions() {
    let mut cumulative_cost = 0.0;
    for cost in [0.25, 0.5] {
        let (mut session, tx) = mock_session();
        tx.send(Ok(result_message(cost))).unwrap();
        let (outcome, spend) = drain_iteration(
            &mut session,
            HeadlessFormat::Plain,
            Some(1.0),
            &mut cumulative_cost,
            &mut Vec::new(),
            &mut Vec::new(),
        )
        .await
        .unwrap();
        assert_eq!(outcome, HeadlessOutcome::Ok);
        assert_eq!(spend.unwrap().cost_usd, cost);
    }
    assert_eq!(cumulative_cost, 0.75);

    let (mut session, tx) = mock_session();
    tx.send(Ok(AgentMessage::UsageUpdate {
        context_tokens_used: 100,
        context_window_size: 1000,
        cost_usd: Some(0.3),
    }))
    .unwrap();
    let mut errors = Vec::new();
    let (outcome, spend) = drain_iteration(
        &mut session,
        HeadlessFormat::Plain,
        Some(1.0),
        &mut cumulative_cost,
        &mut Vec::new(),
        &mut errors,
    )
    .await
    .unwrap();
    assert_eq!(outcome, HeadlessOutcome::BudgetExceeded);
    assert!(
        String::from_utf8(errors)
            .unwrap()
            .contains("$0.3000 >= $0.2500")
    );
    assert!(spend.is_none(), "no result means no invented ledger row");
    assert_eq!(cumulative_cost, 0.75);
}

#[tokio::test]
async fn loop_without_cost_limit_still_accounts_for_recorded_spend() {
    let (mut session, tx) = mock_session();
    tx.send(Ok(result_message(0.5))).unwrap();
    let mut cumulative_cost = 0.25;
    let (outcome, _) = drain_iteration(
        &mut session,
        HeadlessFormat::Json,
        None,
        &mut cumulative_cost,
        &mut Vec::new(),
        &mut Vec::new(),
    )
    .await
    .unwrap();
    assert_eq!(outcome, HeadlessOutcome::Ok);
    assert_eq!(cumulative_cost, 0.75);
}

#[tokio::test]
async fn loop_session_outcomes_reach_the_process_exit_status() {
    const CASE: &str = "_MAGISTR_LOOP_EXIT_CASE";
    if let Ok(case) = std::env::var(CASE) {
        let (mut session, tx) = mock_session();
        let message = if case == "budget" {
            AgentMessage::UsageUpdate {
                context_tokens_used: 100,
                context_window_size: 1000,
                cost_usd: Some(1.0),
            }
        } else {
            let mut message = result_message(0.5);
            if let AgentMessage::Result { is_error, .. } = &mut message {
                *is_error = case == "error";
            }
            message
        };
        tx.send(Ok(message)).unwrap();
        let (outcome, _) = drain_iteration(
            &mut session,
            HeadlessFormat::Plain,
            Some(1.0),
            &mut 0.0,
            &mut Vec::new(),
            &mut Vec::new(),
        )
        .await
        .unwrap();
        eprintln!("loop exit regression reached completion: {case}");
        finish_loop(outcome).unwrap();
        return;
    }

    let thread = std::thread::current();
    let name = thread.name().unwrap();
    for (case, expected) in [("error", 1), ("budget", 2), ("success", 0)] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", name, "--nocapture"])
            .env(CASE, case)
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("loop exit regression reached completion"),
            "{stderr}"
        );
        assert_eq!(output.status.code(), Some(expected), "{case}: {stderr}");
    }
}

#[test]
fn emit_loop_stop_json() {
    let mut buf = Vec::new();
    emit_loop_stop(&HeadlessFormat::Json, "max_iterations", &mut buf).unwrap();
    let output: serde_json::Value = serde_json::from_slice(&buf).unwrap();
    assert_eq!(output["type"], "loop_stop");
    assert_eq!(output["reason"], "max_iterations");
}

#[test]
fn emit_loop_stop_plain() {
    let mut buf = Vec::new();
    emit_loop_stop(&HeadlessFormat::Plain, "spec_complete", &mut buf).unwrap();
    let output = String::from_utf8(buf).unwrap();
    assert!(output.contains("spec_complete"));
}

#[test]
fn emit_iteration_progress_json() {
    let before = make_summary(10, 50);
    let after = make_summary(13, 50);
    let duration = Duration::from_secs(120);

    let mut buf = Vec::new();
    emit_iteration_progress(
        &HeadlessFormat::Json,
        1,
        &HeadlessOutcome::Ok,
        &before,
        &after,
        duration,
        &mut buf,
    )
    .unwrap();

    let output: serde_json::Value = serde_json::from_slice(&buf).unwrap();
    assert_eq!(output["type"], "iteration_progress");
    assert_eq!(output["iteration"], 1);
    assert_eq!(output["status"], "success");
    assert_eq!(output["spec_before"]["checked"], 10);
    assert_eq!(output["spec_after"]["checked"], 13);
}

#[test]
fn emit_iteration_progress_plain() {
    let before = make_summary(10, 50);
    let after = make_summary(13, 50);
    let duration = Duration::from_secs(120);

    let mut buf = Vec::new();
    emit_iteration_progress(
        &HeadlessFormat::Plain,
        2,
        &HeadlessOutcome::Ok,
        &before,
        &after,
        duration,
        &mut buf,
    )
    .unwrap();

    let output = String::from_utf8(buf).unwrap();
    assert!(output.contains("iteration 2"));
    assert!(output.contains("10/50 -> 13/50"));
    assert!(output.contains("success"));
}

#[test]
fn emit_summary_json() {
    let duration = Duration::from_secs(600);
    let mut buf = Vec::new();
    emit_summary(&HeadlessFormat::Json, 5, 10, duration, &mut buf).unwrap();

    let output: serde_json::Value = serde_json::from_slice(&buf).unwrap();
    assert_eq!(output["type"], "loop_summary");
    assert_eq!(output["iterations_completed"], 5);
    assert_eq!(output["max_iterations"], 10);
}

#[test]
fn emit_summary_plain() {
    let duration = Duration::from_secs(600);
    let mut buf = Vec::new();
    emit_summary(&HeadlessFormat::Plain, 5, 10, duration, &mut buf).unwrap();

    let output = String::from_utf8(buf).unwrap();
    assert!(output.contains("5/10 iterations"));
    assert!(output.contains("600.0s"));
}
