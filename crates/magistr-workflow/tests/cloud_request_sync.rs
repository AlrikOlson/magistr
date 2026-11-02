//! The request dogfood loop: a local request capture pushes to the
//! cloud backend. Mirrors the plan proof (cloud_plan_sync.rs).

use std::time::Duration;

use magistr_workflow::cloud::client::CloudClient;
use magistr_workflow::request::RequestEngine;
use magistr_workflow::request::domain::RequestKind;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

mod support;

#[tokio::test]
async fn a_request_capture_pushes_to_the_cloud() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/v1/records"))
        .respond_with(ResponseTemplate::new(201))
        .mount(&server)
        .await;

    let mut engine =
        RequestEngine::new("dogfood".into()).with_cloud(CloudClient::new(server.uri(), "tok"));
    engine
        .capture(RequestKind::Bug, "dana".into(), "it crashes on save".into())
        .unwrap();

    // The push is fire-and-forget on a detached task, so this waits on the
    // request ACTUALLY ARRIVING rather than on a duration guessed against it.
    // The budget bounds failure only — see support::wait_for_request.
    support::wait_for_request(&server, "/v1/records", Duration::from_secs(10))
        .await
        .expect("the capture should have pushed to /v1/records");
}
