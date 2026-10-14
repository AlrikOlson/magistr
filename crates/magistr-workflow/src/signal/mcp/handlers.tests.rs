use super::*;
use crate::roadmap::domain::ChunkStatus;
use crate::signal::domain::SignalStatus;

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
    assert_eq!(parse_kind("feedback").unwrap(), SignalKind::Feedback);
    assert_eq!(parse_kind("bug").unwrap(), SignalKind::Bug);
    assert!(parse_kind("rant").is_err());
}

// ── signal_promote / signal_link (drive the async handlers, assert on the
//    real engine state — the test that matters) ──────────────────────────

use crate::roadmap::engine::RoadmapEngine;
use crate::signal::engine::SignalEngine;
use std::sync::{Arc, Mutex};

/// A SignalService over a fresh signal engine + a shared roadmap engine,
/// with one signal already advanced to `Researched`. Returns (svc, roadmap, id).
fn promotable() -> (SignalService, Arc<Mutex<RoadmapEngine>>, String) {
    let roadmap = Arc::new(Mutex::new(RoadmapEngine::new("proj".into())));
    let mut engine = SignalEngine::new("proj".into());
    let id = engine
        .capture(SignalKind::Idea, "dana".into(), "add a dark theme".into())
        .as_ref()
        .unwrap()
        .value
        .id
        .clone();
    engine.set_status(&id, SignalStatus::Triaged).unwrap();
    engine.set_status(&id, SignalStatus::Researched).unwrap();
    let svc = SignalService::new(engine).with_roadmap(Arc::clone(&roadmap));
    (svc, roadmap, id)
}

#[tokio::test]
async fn promote_creates_chunk_with_bidirectional_refs() {
    let (svc, roadmap, id) = promotable();
    svc.signal_promote(Parameters(PromoteArgs { id: id.clone() }))
        .await
        .unwrap();

    let chunk_id = format!("signal-{id}");
    // The chunk exists, is backlog, and carries signal:<id>.
    {
        let r = roadmap.lock().unwrap();
        let chunk = r
            .roadmap()
            .chunks
            .iter()
            .find(|c| c.id == chunk_id)
            .expect("chunk created");
        assert_eq!(chunk.status, ChunkStatus::Backlog);
        assert!(chunk.cross_refs.contains(&format!("signal:{id}")));
    }
    // The signal is Promoted and carries chunk:<chunk_id>.
    {
        let handle = svc.engine();
        let e = handle.lock().unwrap();
        let s = e.get(&id).unwrap();
        assert_eq!(s.status, SignalStatus::Promoted);
        assert!(s.cross_refs.contains(&format!("chunk:{chunk_id}")));
    }
}

#[tokio::test]
async fn promote_is_idempotent_no_duplicate_chunk() {
    let (svc, roadmap, id) = promotable();
    svc.signal_promote(Parameters(PromoteArgs { id: id.clone() }))
        .await
        .unwrap();
    // Second promote must not create a second chunk.
    svc.signal_promote(Parameters(PromoteArgs { id: id.clone() }))
        .await
        .unwrap();
    let count = roadmap
        .lock()
        .unwrap()
        .roadmap()
        .chunks
        .iter()
        .filter(|c| c.id.starts_with("signal-"))
        .count();
    assert_eq!(count, 1, "re-promote must be a no-op");
}

#[tokio::test]
async fn promote_rejects_unresearched_signal() {
    let roadmap = Arc::new(Mutex::new(RoadmapEngine::new("proj".into())));
    let mut engine = SignalEngine::new("proj".into());
    let id = engine
        .capture(SignalKind::Bug, "x".into(), "boom".into())
        .as_ref()
        .unwrap()
        .value
        .id
        .clone(); // status New
    let svc = SignalService::new(engine).with_roadmap(Arc::clone(&roadmap));

    svc.signal_promote(Parameters(PromoteArgs { id: id.clone() }))
        .await
        .unwrap();
    // No chunk, signal untouched.
    assert!(roadmap.lock().unwrap().roadmap().chunks.is_empty());
    assert_eq!(
        svc.engine().lock().unwrap().get(&id).unwrap().status,
        SignalStatus::New
    );
}

#[tokio::test]
async fn link_attaches_validated_crossref() {
    let mut engine = SignalEngine::new("proj".into());
    let id = engine
        .capture(SignalKind::Question, "x".into(), "why?".into())
        .as_ref()
        .unwrap()
        .value
        .id
        .clone();
    let svc = SignalService::new(engine); // no roadmap needed for link

    svc.signal_link(Parameters(LinkArgs {
        id: id.clone(),
        cross_ref: "think:42".into(),
    }))
    .await
    .unwrap();
    assert!(
        svc.engine()
            .lock()
            .unwrap()
            .get(&id)
            .unwrap()
            .cross_refs
            .contains(&"think:42".to_string())
    );
}
