use super::*;
use crate::plan::domain::TaskStatus;
use crate::request::domain::RequestStatus;

#[test]
fn capture_args_default_when_absent() {
    // Missing fields deserialize to "" (never a -32602); the handler
    // soft-errors on an empty/invalid kind.
    let bare: CaptureArgs = serde_json::from_str(r#"{}"#).unwrap();
    assert_eq!(bare.kind, "");
    assert_eq!(bare.from, "");
    assert_eq!(bare.body, "");
}

#[test]
fn parse_kind_accepts_vocabulary_and_rejects_others() {
    assert_eq!(parse_kind("feedback").unwrap(), RequestKind::Feedback);
    assert_eq!(parse_kind("bug").unwrap(), RequestKind::Bug);
    assert!(parse_kind("rant").is_err());
}

// ── request_promote / request_link (drive the async handlers, assert on the
//    real engine state — the test that matters) ──────────────────────────

use crate::plan::engine::PlanEngine;
use crate::request::engine::RequestEngine;
use std::sync::{Arc, Mutex};

/// A RequestService over a fresh request engine + a shared plan engine,
/// with one request already advanced to `Researched`. Returns (svc, plan, id).
fn promotable() -> (RequestService, Arc<Mutex<PlanEngine>>, String) {
    let plan = Arc::new(Mutex::new(PlanEngine::new("proj".into())));
    let mut engine = RequestEngine::new("proj".into());
    let id = engine
        .capture(
            RequestKind::Suggestion,
            "dana".into(),
            "add a dark theme".into(),
        )
        .as_ref()
        .unwrap()
        .value
        .id
        .clone();
    engine.set_status(&id, RequestStatus::Triaged).unwrap();
    engine.set_status(&id, RequestStatus::Researched).unwrap();
    let svc = RequestService::new(engine).with_plan(Arc::clone(&plan));
    (svc, plan, id)
}

#[tokio::test]
async fn promote_creates_task_with_bidirectional_refs() {
    let (svc, plan, id) = promotable();
    svc.request_promote(Parameters(PromoteArgs { id: id.clone() }))
        .await
        .unwrap();

    let task_id = format!("request-{id}");
    // The task exists, is an idea, and carries request:<id>.
    {
        let r = plan.lock().unwrap();
        let task = r
            .plan()
            .tasks
            .iter()
            .find(|c| c.id == task_id)
            .expect("task created");
        assert_eq!(task.status, TaskStatus::Idea);
        assert!(task.refs.contains(&format!("request:{id}")));
    }
    // The request is Promoted and carries task:<task_id>.
    {
        let handle = svc.engine();
        let e = handle.lock().unwrap();
        let s = e.get(&id).unwrap();
        assert_eq!(s.status, RequestStatus::Promoted);
        assert!(s.refs.contains(&format!("task:{task_id}")));
    }
}

#[tokio::test]
async fn promote_is_idempotent_no_duplicate_task() {
    let (svc, plan, id) = promotable();
    svc.request_promote(Parameters(PromoteArgs { id: id.clone() }))
        .await
        .unwrap();
    // Second promote must not create a second task.
    svc.request_promote(Parameters(PromoteArgs { id: id.clone() }))
        .await
        .unwrap();
    let count = plan
        .lock()
        .unwrap()
        .plan()
        .tasks
        .iter()
        .filter(|c| c.id.starts_with("request-"))
        .count();
    assert_eq!(count, 1, "re-promote must be a no-op");
}

#[tokio::test]
async fn promote_rejects_unresearched_request() {
    let plan = Arc::new(Mutex::new(PlanEngine::new("proj".into())));
    let mut engine = RequestEngine::new("proj".into());
    let id = engine
        .capture(RequestKind::Bug, "x".into(), "boom".into())
        .as_ref()
        .unwrap()
        .value
        .id
        .clone(); // status New
    let svc = RequestService::new(engine).with_plan(Arc::clone(&plan));

    svc.request_promote(Parameters(PromoteArgs { id: id.clone() }))
        .await
        .unwrap();
    // No task, request untouched.
    assert!(plan.lock().unwrap().plan().tasks.is_empty());
    assert_eq!(
        svc.engine().lock().unwrap().get(&id).unwrap().status,
        RequestStatus::New
    );
}

#[tokio::test]
async fn link_attaches_validated_crossref() {
    let mut engine = RequestEngine::new("proj".into());
    let id = engine
        .capture(RequestKind::Question, "x".into(), "why?".into())
        .as_ref()
        .unwrap()
        .value
        .id
        .clone();
    let svc = RequestService::new(engine); // no plan needed for link

    svc.request_link(Parameters(LinkArgs {
        id: id.clone(),
        cross_ref: "decision:42".into(),
    }))
    .await
    .unwrap();
    assert!(
        svc.engine()
            .lock()
            .unwrap()
            .get(&id)
            .unwrap()
            .refs
            .contains(&"decision:42".to_string())
    );
}
