use magistr_engine::native::BlockerKind;
use magistr_engine::{ChunkStatus, NativeConfig, NativeEngines, ProjectAuthority, WriterRole};

use super::*;
use crate::cockpit::{Dock, EngineCmd};
use crate::engine::{EngineEvent, apply};

/// In-memory engines on a throwaway project directory, with one pending
/// chunk the dock could raise a card for.
fn engines(tmp: &tempfile::TempDir) -> NativeEngines {
    let root = tmp.path().join("proj");
    std::fs::create_dir_all(&root).expect("root");
    let config = NativeConfig::from_lookup(&root, |_| None)
        .with_data_dir(tmp.path().join("data"))
        .persist(false);
    let engines =
        ProjectAuthority::admit_for_project(&config, "dock-decisions".to_owned(), WriterRole::Host)
            .expect("host admitted")
            .open();
    engines
        .roadmap_add_chunk(
            "auth".to_owned(),
            "Authenticate users".to_owned(),
            String::new(),
            ChunkStatus::Pending,
            200,
            String::new(),
            vec![],
            vec![],
            None,
            None,
        )
        .expect("the chunk");
    engines
}

/// The frame `cmd` answers with, or the port's refusal as a panic —
/// these are the real engines, so a refusal is a test failure.
fn frame_of(port: &mut NativeEnginePort, cmd: &EngineCmd) -> magistr_engine::NativeFrame {
    match apply(port, cmd) {
        EngineEvent::Frame(frame) => frame,
        other => panic!("the engines refused {cmd:?}: {other:?}"),
    }
}

#[test]
fn an_accepted_reprioritize_moves_the_chunk_and_the_frame_takes_the_card() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let engines = engines(&tmp);
    engines
        .roadmap_reprioritize("auth", 100, "blocking the release".to_owned())
        .expect("the proposal");

    // The proposal is a card on the dock, the way the feed's frame puts
    // it there.
    let mut dock = Dock::default();
    let chunk = engines.roadmap_chunk("auth").expect("the chunk");
    dock.apply(&chunk_changed(&chunk));
    assert_eq!(dock.len(), 1, "the proposal is waiting for an answer");

    let mut port = NativeEnginePort::new(engines.clone());
    let frame = frame_of(
        &mut port,
        &EngineCmd::ResolveProposal {
            chunk: "auth".to_owned(),
            kind: ProposalKind::Reprioritize,
            accept: true,
        },
    );

    // The store moved…
    let after = engines.roadmap_chunk("auth").expect("the chunk");
    assert_eq!(after.priority, 100, "the suggestion was applied");
    assert!(after.reprioritize.is_none(), "and the proposal is disposed");
    // …and the frame the answer came back as is what takes the card.
    assert_eq!(frame.kind, "chunk_changed");
    dock.apply(&frame);
    assert!(dock.is_empty(), "the card resolved on the engines' answer");
}

#[test]
fn a_rejected_reprioritize_disposes_the_proposal_and_leaves_the_priority() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let engines = engines(&tmp);
    engines
        .roadmap_reprioritize("auth", 100, "blocking the release".to_owned())
        .expect("the proposal");

    let mut port = NativeEnginePort::new(engines.clone());
    let frame = frame_of(
        &mut port,
        &EngineCmd::ResolveProposal {
            chunk: "auth".to_owned(),
            kind: ProposalKind::Reprioritize,
            accept: false,
        },
    );
    let after = engines.roadmap_chunk("auth").expect("the chunk");
    assert_eq!(after.priority, 200, "a rejection applies nothing");
    assert!(after.reprioritize.is_none(), "and clears the proposal");
    assert!(frame.payload["chunk"]["reprioritize"].is_null());
}

#[test]
fn a_cleared_blocker_lifts_blocked_by_and_the_frame_takes_the_card() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let engines = engines(&tmp);
    engines
        .roadmap_set_blocked_by(
            "auth",
            BlockerKind::AwaitingHuman,
            "needs a person to pick the scheme".to_owned(),
            None,
        )
        .expect("set");

    let mut dock = Dock::default();
    let chunk = engines.roadmap_chunk("auth").expect("the chunk");
    dock.apply(&chunk_changed(&chunk));
    assert_eq!(dock.len(), 1, "the blocker is waiting for an answer");

    let mut port = NativeEnginePort::new(engines.clone());
    let frame = frame_of(
        &mut port,
        &EngineCmd::ClearBlocker {
            chunk: "auth".to_owned(),
        },
    );
    assert!(
        engines
            .roadmap_chunk("auth")
            .expect("the chunk")
            .blocked_by
            .is_none(),
        "the blocker is lifted in the store"
    );
    dock.apply(&frame);
    assert!(dock.is_empty(), "and the frame took the card");

    // The engines refuse a second clear rather than answer a no-op, and
    // the cockpit hears it as words for the stage.
    assert!(matches!(
        apply(
            &mut port,
            &EngineCmd::ClearBlocker {
                chunk: "auth".to_owned(),
            }
        ),
        EngineEvent::PortFailed {
            cmd: EngineCmd::ClearBlocker { chunk },
            ..
        } if chunk == "auth"
    ));
}

/// The broadcast frame the external server writes for a changed chunk —
/// what the dock folds when the feed, rather than the port, reports.
fn chunk_changed(chunk: &magistr_engine::native::Chunk) -> magistr_engine::NativeFrame {
    let payload = serde_json::to_value(chunk).expect("a chunk serializes");
    magistr_engine::NativeFrame {
        family: "roadmap".to_owned(),
        kind: "chunk_changed".to_owned(),
        payload: serde_json::json!({"family": "roadmap", "type": "chunk_changed",
            "chunk": payload}),
    }
}
