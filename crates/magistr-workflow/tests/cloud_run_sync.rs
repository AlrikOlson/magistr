//! The run dogfood loop (progress slice): recording a run progress
//! pushes its envelope to the cloud backend. Mirrors the roadmap/signal proofs.

use std::time::Duration;

use magistr_workflow::cloud::client::CloudClient;
use magistr_workflow::run::domain::progress::ProgressType;
use magistr_workflow::run::domain::step::StepType;
use magistr_workflow::run::engine::RunEngine;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

mod support;

#[tokio::test]
async fn a_run_progress_pushes_to_the_cloud() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/v1/records"))
        .respond_with(ResponseTemplate::new(201))
        .mount(&server)
        .await;

    let mut engine =
        RunEngine::new("dogfood".into()).with_cloud(CloudClient::new(server.uri(), "tok"));
    // The goal/step frames are id-only and don't push; only the progress does.
    engine
        .set_goal("o".into(), vec![], vec![], "s".into())
        .unwrap();
    engine
        .add_step("t1".into(), "Step".into(), StepType::Implement, None, None)
        .unwrap();
    engine.start_step("t1").expect("start");
    engine
        .record_progress(
            Some("t1"),
            ProgressType::Code,
            "did the thing".into(),
            vec![],
            vec!["Edit".into()],
            String::new(),
            Some(907),
        )
        .expect("record_progress");

    // The push is fire-and-forget on a detached step, so this waits on the
    // request ACTUALLY ARRIVING rather than on a duration guessed against it.
    // The budget bounds failure only — see support::wait_for_request.
    support::wait_for_request(&server, "/v1/records", Duration::from_secs(10))
        .await
        .expect("the progress should have pushed to /v1/records");
}
