//! Cloud reasoning import preserves authored identities and exposes local commit failures.

use std::sync::{Arc, Mutex};

use magistr_workflow::cloud::build::from_note;
use magistr_workflow::cloud::client::CloudClient;
use magistr_workflow::cloud::pull::{apply_decision_records, reconcile_all, reconcile_decision};
use magistr_workflow::decision::config::DecisionConfig;
use magistr_workflow::decision::domain::{NextAction, Note};
use magistr_workflow::decision::engine::core::ReasoningServer;
use magistr_workflow::plan::PlanEngine;
use magistr_workflow::request::RequestEngine;
use serde_json::{Value, json};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn quiet_server() -> ReasoningServer {
    let mut c = DecisionConfig::default();
    c.display.color_output = false;
    // Same project the fixtures stamp — reconcile filters by record.project_id
    // (sync-project-scope), so the engine and the envelopes must agree.
    ReasoningServer::new_for_project(c, "t".into())
}

fn note(n: u32, thought: &str) -> Note {
    Note {
        note: n,
        estimated_total: 10,
        purpose: format!("note {n}"),
        context: "ctx".into(),
        thought: thought.into(),
        outcome: "out".into(),
        next_action: NextAction::Text("next".into()),
        rationale: "why".into(),
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
        record_id: Some(format!("record-{n}")),
    }
}

fn envelope_for(n: u32, thought: &str) -> Value {
    serde_json::to_value(from_note("t", &note(n, thought)).unwrap()).unwrap()
}

#[test]
fn adopts_absent_notes_and_sorts_the_log() {
    let mut engine = quiet_server();
    engine.adopt_notes(vec![note(5, "local five")]).unwrap();

    let adopted = apply_decision_records(
        &mut engine,
        &[
            envelope_for(3, "cloud three"),
            envelope_for(7, "cloud seven"),
        ],
    );
    assert_eq!(adopted.unwrap(), 2);

    let numbers: Vec<u32> = engine.history().notes.iter().map(|s| s.note).collect();
    assert_eq!(numbers, vec![3, 5, 7], "log stays sorted by note_number");
}

/// THE RACE (decision twin of reconcile-recency-guard): a refresh delivers the
/// cloud copy of a note this process already holds — possibly staler than the
/// local copy. The local note must survive untouched.
#[test]
fn an_existing_local_note_is_never_replaced_by_a_cloud_copy() {
    let mut engine = quiet_server();
    engine
        .adopt_notes(vec![note(42, "local, freshly revised reasoning")])
        .unwrap();

    let adopted = apply_decision_records(&mut engine, &[envelope_for(42, "stale cloud copy")]);
    assert!(
        adopted.is_err(),
        "conflicting authored content must be reported"
    );

    let s = &engine.history().notes[0];
    assert_eq!(s.thought, "local, freshly revised reasoning");
}

#[test]
fn malformed_local_records_refuse_the_batch_before_any_commit() {
    let mut engine = quiet_server();
    let adopted = apply_decision_records(
        &mut engine,
        &[
            // Stamped for this project so it reaches (and fails) deserialization.
            json!({ "record": { "note": "not-a-number", "project_id": "t" } }),
            json!({ "no_record": true }),
            envelope_for(1, "good"),
        ],
    );
    assert!(matches!(
        adopted,
        Err(magistr_workflow::cloud::client::CloudError::Local { saved: 0, .. })
    ));
    assert!(engine.history().notes.is_empty());
}

/// sync-project-scope: an org tenant holds EVERY project's records; a note
/// stamped for another project — or an unstamped legacy record whose origin
/// is unknowable — must never merge in. Note numbers are project-global, so
/// adopting a foreign note silently poisons this project's numbering (a
/// bleed observed live between two projects sharing one tenant).
#[test]
fn foreign_and_unstamped_records_never_merge() {
    let mut engine = quiet_server();
    let foreign =
        serde_json::to_value(from_note("other-project", &note(9, "theirs")).unwrap()).unwrap();
    let mut unstamped = envelope_for(11, "pre-stamp legacy");
    unstamped["record"]
        .as_object_mut()
        .unwrap()
        .remove("project_id");

    let adopted =
        apply_decision_records(&mut engine, &[foreign, unstamped, envelope_for(2, "ours")]);
    assert_eq!(
        adopted.unwrap(),
        1,
        "only this project's stamped record merges"
    );
    let numbers: Vec<u32> = engine.history().notes.iter().map(|s| s.note).collect();
    assert_eq!(numbers, vec![2]);
}

#[tokio::test]
async fn reconcile_decision_pulls_cloud_notes_into_the_local_engine() {
    let server = MockServer::start().await;
    let records: Vec<Value> = vec![envelope_for(1, "one"), envelope_for(2, "two")];
    Mock::given(method("GET"))
        .and(path("/v1/records"))
        .and(query_param("family", "decision"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "records": records })))
        .mount(&server)
        .await;

    let client = CloudClient::new(server.uri(), "tok");
    let mut engine = quiet_server();

    assert_eq!(reconcile_decision(&client, &mut engine).await.unwrap(), 2);
    assert_eq!(engine.history().notes.len(), 2);

    // Re-pull is a no-op (insert-if-absent), not a duplicate.
    assert_eq!(reconcile_decision(&client, &mut engine).await.unwrap(), 0);
    assert_eq!(engine.history().notes.len(), 2);
}

/// The startup hydrate: one call converges all three pull-able families, and
/// a failing family (request here: 500) never blocks the others.
#[tokio::test]
async fn reconcile_all_hydrates_every_family_and_tolerates_one_failing() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/records"))
        .and(query_param("family", "decision"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({ "records": [envelope_for(1, "one")] })),
        )
        .mount(&server)
        .await;
    let task = json!({
        "record": {
            "id": "c1", "title": "T", "status": "idea", "priority": 1,
            "description": "", "notes": "", "acceptance": [], "deps": [],
            "refs": [], "shared": false, "project_id": "t",
            "created_at": "2026-06-08T00:00:00Z", "updated_at": "2026-06-08T00:00:00Z"
        }
    });
    Mock::given(method("GET"))
        .and(path("/v1/records"))
        .and(query_param("family", "plan"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "records": [task] })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/records"))
        .and(query_param("family", "request"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let client = CloudClient::new(server.uri(), "tok");
    let decision = Arc::new(Mutex::new(quiet_server()));
    let plan = Arc::new(Mutex::new(PlanEngine::new("t".into())));
    let request = Arc::new(Mutex::new(RequestEngine::new("t".into())));

    let (t, r, s) = reconcile_all(&client, &decision, &plan, &request).await;
    assert_eq!((t.unwrap(), r.unwrap()), (1, 1));
    assert!(s.is_err());
    assert_eq!(decision.lock().unwrap().history().notes.len(), 1);
    assert_eq!(plan.lock().unwrap().plan().tasks.len(), 1);
}

#[test]
fn distinct_equal_number_and_prose_records_are_both_retained() {
    let mut engine = quiet_server();
    let first = note(1, "same authored words");
    let mut second = first.clone();
    second.record_id = Some("different-record".into());
    assert_eq!(engine.adopt_notes(vec![first, second]).unwrap().value, 2);
    let history = engine.history();
    assert_eq!(history.notes.len(), 2);
    assert_ne!(history.notes[0].record_id, history.notes[1].record_id);
    assert_ne!(history.notes[0].note, history.notes[1].note);
}

#[test]
fn colliding_import_numbers_remap_their_own_references() {
    use magistr_workflow::decision::domain::DepEdge;
    let mut engine = quiet_server();
    engine.adopt_notes(vec![note(1, "local")]).unwrap();
    let mut origin = note(1, "remote origin");
    origin.record_id = Some("remote-origin".into());
    let mut dependent = note(2, "remote continuation");
    dependent.record_id = Some("remote-dependent".into());
    dependent.dependencies = Some(vec![DepEdge::Bare(1)]);
    engine.adopt_notes(vec![origin, dependent]).unwrap();
    let origin = engine
        .history()
        .notes
        .iter()
        .find(|note| note.record_id.as_deref() == Some("remote-origin"))
        .unwrap();
    let dependent = engine
        .history()
        .notes
        .iter()
        .find(|note| note.record_id.as_deref() == Some("remote-dependent"))
        .unwrap();
    assert_ne!(origin.note, 1);
    assert_eq!(
        dependent.dependencies.as_ref().unwrap()[0].note(),
        origin.note
    );
}

#[test]
fn ambiguous_import_references_refuse_the_entire_batch() {
    use magistr_workflow::decision::domain::DepEdge;
    let mut engine = quiet_server();
    let first = note(1, "first origin");
    let mut other = note(1, "another origin");
    other.record_id = Some("another-origin".into());
    let mut dependent = note(2, "which origin");
    dependent.dependencies = Some(vec![DepEdge::Bare(1)]);
    assert!(engine.adopt_notes(vec![first, other, dependent]).is_err());
    assert!(engine.history().notes.is_empty());
}

#[test]
fn known_identity_cannot_hide_a_changed_authored_reference() {
    use magistr_workflow::decision::domain::DepEdge;
    let mut engine = quiet_server();
    let first = note(1, "origin");
    let second = note(2, "other");
    let mut dependent = note(3, "dependent");
    dependent.dependencies = Some(vec![DepEdge::Bare(1)]);
    engine
        .adopt_notes(vec![first, second, dependent.clone()])
        .unwrap();
    dependent.dependencies = Some(vec![DepEdge::Bare(2)]);
    assert!(engine.adopt_notes(vec![dependent]).is_err());
    assert_eq!(
        engine.history().notes[2].dependencies.as_ref().unwrap()[0].note(),
        1
    );
}

#[test]
fn local_commit_failure_does_not_count_imported_records() {
    let directory = tempfile::tempdir().unwrap();
    let mut config = DecisionConfig::default();
    config.persistence.enabled = true;
    config.persistence.data_dir = directory.path().to_path_buf();
    let persistence =
        magistr_workflow::decision::persistence::Persistence::for_project(&config.persistence, "t");
    let mut engine = ReasoningServer::new_for_project(config, "t".into());
    engine.adopt_notes(vec![note(1, "existing")]).unwrap();
    let path = persistence.store_path().unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let lock = path.with_extension("json.lock");
    std::fs::remove_file(&lock).unwrap();
    std::fs::create_dir(&lock).unwrap();
    let error = apply_decision_records(&mut engine, &[envelope_for(2, "incoming")]).unwrap_err();
    assert!(matches!(
        error,
        magistr_workflow::cloud::client::CloudError::Local {
            saved: 0,
            store: Some(_),
            ..
        }
    ));
    assert_eq!(engine.history().notes.len(), 1);
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}
