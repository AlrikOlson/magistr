//! Request pull/reconcile: the cloud's requests merge into the local
//! engine. NEWEST-wins per request via lifecycle progress (reconcile-recency-guard):
//! the forward-only status rank + append-only enrichment count decide, exactly as
//! in `merge_request_stores` — a stale cloud copy must never roll back a newer
//! local transition. Mirror of the plan pull proof.

use magistr_workflow::cloud::build::from_request;
use magistr_workflow::cloud::client::CloudClient;
use magistr_workflow::cloud::pull::reconcile_requests;
use magistr_workflow::request::RequestEngine;
use magistr_workflow::request::domain::{Request, RequestKind, RequestStatus};
use serde_json::{Value, json};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn request(id: &str) -> Request {
    request_at(id, RequestStatus::New)
}

/// A request that PROVABLY belongs to `project`. Ownership must be stated in the
/// fixture: the push no longer fills it in for an unstamped request, so letting
/// it default would mean asserting the laundering rather than the guard.
fn owned_request(id: &str, project: &str) -> Request {
    Request {
        project_id: Some(project.into()),
        ..request(id)
    }
}

fn request_at(id: &str, status: RequestStatus) -> Request {
    Request {
        id: id.into(),
        kind: RequestKind::Bug,
        from: "dana".into(),
        body: format!("body {id}"),
        content: None,
        created: "2026-06-08T00:00:00Z".into(),
        status,
        enrichment: vec![],
        refs: vec![],
        raised_at: None,
        snooze_until: None,
        project_id: None,
    }
}

fn upsert(engine: &mut RequestEngine, mut request: Request) {
    request.project_id = Some("t".into());
    engine.upsert_request(request).unwrap();
}

#[test]
fn upsert_request_replaces_by_id_or_inserts() {
    let mut engine = RequestEngine::new("t".into());
    upsert(&mut engine, request("a"));
    upsert(&mut engine, request("b"));
    assert_eq!(engine.requests().requests.len(), 2);

    // A PROGRESSED cloud copy replaces 'a' — no duplicate.
    upsert(&mut engine, request_at("a", RequestStatus::Triaged));
    assert_eq!(engine.requests().requests.len(), 2);
    let a = engine
        .requests()
        .requests
        .iter()
        .find(|s| s.id == "a")
        .unwrap();
    assert_eq!(a.status, RequestStatus::Triaged);
}

/// The plan race's request twin: a stale cloud copy (still New) must not
/// roll back a local request that already advanced to Triaged.
#[test]
fn upsert_request_keeps_newer_local_lifecycle_progress() {
    let mut engine = RequestEngine::new("t".into());
    upsert(&mut engine, request_at("a", RequestStatus::Triaged));

    // Reconcile delivers the cloud copy from before the transition's push landed.
    upsert(&mut engine, request_at("a", RequestStatus::New));

    let a = engine
        .requests()
        .requests
        .iter()
        .find(|s| s.id == "a")
        .unwrap();
    assert_eq!(
        a.status,
        RequestStatus::Triaged,
        "a stale cloud copy rolled back a local lifecycle transition"
    );
}

/// sync-project-scope, the request family's copy: with an org token every
/// project in the org shares one cloud tenant, so `apply_request_records` filters
/// on the record's project stamp exactly as plan and decision do.
///
/// This check is here because it was NOT: neutralising `record_is_local` reddened
/// the plan and decision files and left this one entirely green, so the one line
/// standing between the request store and every other project's requests could
/// have been deleted to applause. The rule is per-FAMILY, and so is its proof.
#[test]
fn foreign_and_unstamped_requests_never_merge() {
    use magistr_workflow::cloud::pull::apply_request_records;

    let mut engine = RequestEngine::new("ours".into());

    let foreign = serde_json::to_value(from_request(
        "other-project",
        &owned_request("theirs", "other-project"),
    ))
    .expect("foreign envelope");
    // Unstamped now needs no surgery: a request with no origin reaches the wire
    // explicitly unprovable, which is exactly the legacy record this models.
    let unstamped =
        serde_json::to_value(from_request("ours", &request("legacy"))).expect("legacy envelope");
    let ours = serde_json::to_value(from_request("ours", &owned_request("mine", "ours")))
        .expect("our envelope");

    let merged = apply_request_records(&mut engine, &[foreign, unstamped, ours]).unwrap();

    // Load-bearing first: which requests actually landed. An earlier count
    // assertion would fail under the same deliberate breakage and hide which
    // half broke.
    let ids: Vec<String> = engine
        .requests()
        .requests
        .iter()
        .map(|s| s.id.clone())
        .collect();
    assert_eq!(
        ids,
        vec!["mine".to_string()],
        "only this project's stamped request may merge — a foreign or unattributable one is the bleed"
    );
    assert_eq!(merged, 1);
}

#[tokio::test]
async fn reconcile_requests_pulls_cloud_requests_into_the_local_engine() {
    let server = MockServer::start().await;
    let records: Vec<Value> = ["a", "b"]
        .iter()
        .map(|id| serde_json::to_value(from_request("t", &owned_request(id, "t"))).unwrap())
        .collect();
    Mock::given(method("GET"))
        .and(path("/v1/records"))
        .and(query_param("family", "request"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "records": records })))
        .mount(&server)
        .await;

    let client = CloudClient::new(server.uri(), "tok");
    let mut engine = RequestEngine::new("t".into());

    assert_eq!(reconcile_requests(&client, &mut engine).await.unwrap(), 2);
    assert_eq!(engine.requests().requests.len(), 2);

    // Re-pull acknowledges no new records.
    assert_eq!(reconcile_requests(&client, &mut engine).await.unwrap(), 0);
    assert_eq!(engine.requests().requests.len(), 2);
}
