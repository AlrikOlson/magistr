use super::*;
use crate::roadmap::RoadmapEngine;

use std::sync::{Arc, Barrier, Mutex};

use rmcp::handler::server::wrapper::Parameters;

use crate::signal::SignalService;
use crate::signal::engine::SignalEngine;
use crate::signal::mcp::handlers::PromoteArgs;

fn promotable() -> (SignalUseCases, Arc<Mutex<RoadmapEngine>>, Signal) {
    let roadmap = Arc::new(Mutex::new(RoadmapEngine::new("project".into())));
    let app = SignalUseCases::new(Arc::new(Mutex::new(SignalEngine::new("project".into()))))
        .with_roadmap(Arc::clone(&roadmap));
    let captured = app
        .capture(
            SignalKind::Idea,
            "user".into(),
            "A useful change".into(),
            None,
        )
        .unwrap();
    let signal = app
        .research(&captured.value.id, "Verified".into(), 0.9, vec![], None)
        .unwrap();
    (app, roadmap, signal.value)
}

#[test]
fn concurrent_native_promotions_create_one_reciprocally_linked_chunk() {
    let (app, roadmap, signal) = promotable();
    let barrier = Arc::new(Barrier::new(2));
    let results = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..2)
            .map(|_| {
                let app = app.clone();
                let barrier = Arc::clone(&barrier);
                let id = signal.id.clone();
                scope.spawn(move || {
                    barrier.wait();
                    app.promote(&id).unwrap()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(results.iter().filter(|result| result.created).count(), 1);
    let chunk_id = chunk_id_for(&signal.id);
    let roadmap = roadmap.lock().unwrap();
    assert_eq!(roadmap.roadmap().chunks.len(), 1);
    assert_eq!(
        roadmap.roadmap().chunks[0].cross_refs,
        [format!("signal:{}", signal.id)]
    );
    for result in results {
        assert_eq!(result.signal.value.status, SignalStatus::Promoted);
        assert_eq!(
            result.signal.value.cross_refs,
            [format!("chunk:{chunk_id}")]
        );
    }
}

#[test]
fn retries_recover_both_partial_promotion_boundaries() {
    for already_promoted in [false, true] {
        let (app, roadmap, signal) = promotable();
        let chunk_id = chunk_id_for(&signal.id);
        {
            let mut roadmap = roadmap.lock().unwrap();
            roadmap
                .add_chunk(
                    chunk_id.clone(),
                    "Change".into(),
                    ChunkStatus::Backlog,
                    0,
                    signal.body.clone(),
                    vec![],
                    vec![],
                    false,
                )
                .unwrap();
            roadmap
                .link_chunk(&chunk_id, &format!("signal:{}", signal.id))
                .unwrap();
        }
        if already_promoted {
            app.engine().lock().unwrap().promote(&signal.id).unwrap();
        }
        let result = app.promote(&signal.id).unwrap();
        assert_eq!(result.signal.value.status, SignalStatus::Promoted);
        assert!(
            result
                .signal
                .value
                .cross_refs
                .contains(&format!("chunk:{chunk_id}"))
        );
        assert!(!app.promote(&signal.id).unwrap().created);
        assert_eq!(roadmap.lock().unwrap().roadmap().chunks.len(), 1);
    }
}

#[tokio::test]
async fn native_and_mcp_promotion_return_both_committed_owners() {
    let (app, roadmap, signal) = promotable();
    let service = SignalService::from_use_cases(app.clone());
    let fresh = service
        .signal_promote(Parameters(PromoteArgs {
            id: signal.id.clone(),
        }))
        .await
        .unwrap()
        .structured_content
        .unwrap();
    assert_eq!(fresh["created"], true);
    assert!(fresh.get("signal").is_some());
    assert!(fresh["chunk"].get("source").is_some());
    assert!(fresh["signal"].get("source").is_some());
    assert!(!app.promote(&signal.id).unwrap().created);
    let retry = service
        .signal_promote(Parameters(PromoteArgs {
            id: signal.id.clone(),
        }))
        .await
        .unwrap()
        .structured_content
        .unwrap();
    assert_eq!(retry["created"], false);
    assert!(retry["signal"].get("source").is_some());
    assert_eq!(retry["chunk"]["value"]["id"], chunk_id_for(&signal.id));
    assert_eq!(roadmap.lock().unwrap().roadmap().chunks.len(), 1);
}

#[test]
fn invalid_state_and_unwired_promotion_leave_native_state_unchanged() {
    let engine = Arc::new(Mutex::new(SignalEngine::new("project".into())));
    let app = SignalUseCases::new(engine);
    let signal = app
        .capture(SignalKind::Bug, "user".into(), "Broken".into(), None)
        .unwrap();
    assert!(matches!(
        app.promote(&signal.value.id),
        Err(SignalError::NotWired)
    ));
    let roadmap = Arc::new(Mutex::new(RoadmapEngine::new("project".into())));
    let app = app.with_roadmap(Arc::clone(&roadmap));
    assert!(matches!(
        app.promote(&signal.value.id),
        Err(SignalError::InvalidState(_))
    ));
    assert!(roadmap.lock().unwrap().roadmap().chunks.is_empty());
    assert_eq!(app.get(&signal.value.id).unwrap(), signal.value);
    assert_eq!(chunk_id_for("f1c9a4e2-1234"), "signal-f1c9a4e2-1234");
}
