//! The decision dogfood loop: recording a reasoning note pushes its
//! envelope to the cloud backend. Completes local→cloud push for all 4 families.

use std::time::Duration;

use magistr_workflow::cloud::client::CloudClient;
use magistr_workflow::decision::config::DecisionConfig;
use magistr_workflow::decision::domain::{NextAction, Note};
use magistr_workflow::decision::engine::core::ReasoningServer;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

mod support;

fn quiet_config() -> DecisionConfig {
    let mut c = DecisionConfig::default();
    c.display.color_output = false;
    c
}

fn note(n: u32) -> Note {
    Note {
        note: n,
        estimated_total: 5,
        purpose: "analysis".into(),
        context: format!("ctx {n}"),
        thought: format!("thought {n}"),
        outcome: format!("outcome {n}"),
        next_action: NextAction::Text(format!("next {n}")),
        rationale: format!("rationale {n}"),
        confidence: None,
        uncertainty_notes: None,
        revises_note: None,
        revision_reason: None,
        revised_by: None,
        is_final_note: None,
        thread_from: None,
        thread_id: None,
        thread_name: None,
        tools_used: None,
        dependencies: None,
        timestamp: None,
        duration_ms: None,
        log_id: None,
        pinned: None,
        cwd: None,
        run_ref: None,
        record_id: None,
    }
}

#[tokio::test]
async fn a_note_pushes_to_the_cloud() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/v1/records"))
        .respond_with(ResponseTemplate::new(201))
        .mount(&server)
        .await;

    let mut engine =
        ReasoningServer::new(quiet_config()).with_cloud(CloudClient::new(server.uri(), "tok"));
    engine.process_note_native(note(1)).expect("process_note");

    // The push is fire-and-forget on a detached task, so this waits on the
    // request ACTUALLY ARRIVING rather than on a duration guessed against it.
    // The budget bounds failure only — see support::wait_for_request.
    support::wait_for_request(&server, "/v1/records", Duration::from_secs(10))
        .await
        .expect("the recorded note should have pushed to /v1/records");
}
