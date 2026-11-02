//! The plan dogfood loop: a local task mutation pushes to the
//! cloud backend. The wiremock test proves the engine→cloud wiring
//! deterministically; the #[ignore]'d test confirms it against a live deploy.

use std::time::Duration;

use magistr_workflow::cloud::client::CloudClient;
use magistr_workflow::plan::PlanEngine;
use magistr_workflow::plan::domain::TaskStatus;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

mod support;

#[tokio::test]
async fn a_plan_mutation_pushes_to_the_cloud() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/v1/records"))
        .respond_with(ResponseTemplate::new(201))
        .mount(&server)
        .await;

    let mut engine =
        PlanEngine::new("dogfood".into()).with_cloud(CloudClient::new(server.uri(), "tok"));
    engine
        .add_task(
            "c1".into(),
            "Title".into(),
            TaskStatus::Idea,
            1,
            String::new(),
            vec![],
            vec![],
            false,
        )
        .expect("add_task");

    // The push is fire-and-forget on a detached task, so this waits on the
    // request ACTUALLY ARRIVING rather than on a duration guessed against it.
    // The budget bounds failure only — see support::wait_for_request.
    support::wait_for_request(&server, "/v1/records", Duration::from_secs(10))
        .await
        .expect("the mutation should have pushed to /v1/records");
}

/// Live end-to-end: mutate a task locally with a real cloud client, then read
/// it back from the cloud to prove it landed. Gated on env so it never runs
/// unprompted (the engine's project_id must equal the token's `tenant` claim).
#[tokio::test]
#[ignore = "requires MAGISTR_WORKFLOW_CLOUD_URL/_TOKEN (a live backend)"]
async fn a_plan_mutation_reaches_the_live_cloud() {
    let url = std::env::var("MAGISTR_WORKFLOW_CLOUD_URL").expect("MAGISTR_WORKFLOW_CLOUD_URL");
    let token =
        std::env::var("MAGISTR_WORKFLOW_CLOUD_TOKEN").expect("MAGISTR_WORKFLOW_CLOUD_TOKEN");
    let tenant =
        std::env::var("MAGISTR_WORKFLOW_CLOUD_TENANT").unwrap_or_else(|_| "rust-live".into());
    let id = format!("dogfood-{}", std::process::id());

    let mut engine =
        PlanEngine::new(tenant).with_cloud(CloudClient::new(url.clone(), token.clone()));
    engine
        .add_task(
            id.clone(),
            "live".into(),
            TaskStatus::Idea,
            1,
            String::new(),
            vec![],
            vec![],
            false,
        )
        .expect("add_task");
    // Same shape as the wiremock twin, one hop further out: the condition is
    // the cloud actually SERVING the task back. A fixed sleep here was worse
    // than in the mock test, because a real network's tail is unbounded — the
    // one place a guessed constant is guaranteed to be wrong eventually.
    let endpoint = format!("{}/v1/records/plan/task/{id}", url.trim_end_matches('/'));
    let http = reqwest::Client::new();
    support::wait_until(
        Duration::from_secs(30),
        "the task becoming readable",
        || async {
            http.get(&endpoint)
                .bearer_auth(&token)
                .send()
                .await
                .is_ok_and(|r| r.status().as_u16() == 200)
        },
    )
    .await
    .expect("the task should be readable in the cloud");
}
