use magistr_engine::native::BlockerKind;
use magistr_engine::{
    ChunkStatus, Delivery, NativeConfig, NativeEngines, NativeEvent, ProjectAuthority,
    RoadmapFrame, WriterRole,
};

use super::*;
use magistr_cockpit::cockpit::{Dock, EngineCmd};
use magistr_cockpit::engine::{EngineEvent, apply};

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

/// The event `cmd` answers with, or the port's refusal as a panic —
/// these are the real engines, so a refusal is a test failure.
fn frame_of(port: &mut NativeEnginePort, cmd: &EngineCmd) -> NativeEvent {
    match apply(port, cmd) {
        EngineEvent::Event {
            event,
            delivery: Delivery::Local,
        } => event,
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
    // …and the event the answer came back as is what takes the card.
    assert!(matches!(
        &frame,
        NativeEvent::Roadmap(RoadmapFrame::ChunkChanged { chunk }) if chunk.id == "auth"
    ));
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
    assert!(matches!(
        frame,
        NativeEvent::Roadmap(RoadmapFrame::ChunkChanged { chunk }) if chunk.reprioritize.is_none()
    ));
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

/// The event the external server announces for a changed chunk — what
/// the dock folds when the feed, rather than the port, reports.
fn chunk_changed(chunk: &magistr_engine::native::Chunk) -> NativeEvent {
    NativeEvent::Roadmap(RoadmapFrame::ChunkChanged {
        chunk: chunk.clone(),
    })
}
use magistr_cockpit::WidgetId;
use magistr_cockpit::cockpit::{Cmd, Model, Msg, Size, StageTab, Widget, update};
use magistr_cockpit::input::{Button, KeyCode, KeyEvent, MouseEvent, MouseKind};

fn model() -> Model {
    Model::new(Size::new(80, 24)).with_tab(StageTab::Turn)
}

fn key(code: KeyCode) -> Msg {
    Msg::Key(KeyEvent::new(code))
}

fn frame(family: &str, kind: &str, payload: serde_json::Value) -> Msg {
    Msg::Engine(EngineEvent::local(
        magistr_cockpit::harness::fixtures::frame(family, kind, payload),
    ))
}

fn blocker_frame(id: &str) -> Msg {
    frame(
        "roadmap",
        "chunk_changed",
        serde_json::json!({"chunk": {
            "id": id,
            "blocked_by": {"kind": "awaiting_human", "reason": "needs a decision"},
        }}),
    )
}

fn proposal_frame(id: &str) -> Msg {
    frame(
        "roadmap",
        "chunk_changed",
        serde_json::json!({"chunk": {
            "id": id,
            "reprioritize": {"suggested_priority": 300, "reason": "later"},
        }}),
    )
}

fn left_click(target: WidgetId) -> Msg {
    Msg::Mouse {
        event: MouseEvent::new(MouseKind::Down(Button::Left), 10, 10),
        target: Some(target),
    }
}

/// A stale dock card names a decision the real engine cannot resolve.
fn refused_answer(m: &mut Model, cmd: &Cmd) -> String {
    let Cmd::Engine(cmd) = cmd else {
        panic!("a dock answer must reach the engine");
    };
    let tmp = tempfile::tempdir().expect("throwaway store");
    let config = magistr_engine::NativeConfig::from_lookup(tmp.path(), |_| None)
        .with_data_dir(tmp.path().join("data"))
        .persist(false);
    let engines = magistr_engine::ProjectAuthority::admit_for_project(
        &config,
        "refused-dock".to_owned(),
        magistr_engine::WriterRole::Host,
    )
    .expect("host admitted")
    .open();
    let mut port = NativeEnginePort::new(engines);
    let event = magistr_cockpit::engine::apply(&mut port, cmd);
    let EngineEvent::PortFailed { what, error, .. } = &event else {
        panic!("a missing decision must be refused");
    };
    let line = format!("engine {what}: {error}");
    update(m, Msg::Engine(event));
    line
}

fn assert_refusal_error(m: &Model, error: &str) {
    assert!(m.turn().lines().iter().any(|line| *line.kind()
        == magistr_cockpit::turn::LineKind::Error
        && line.text() == error));
}

#[test]
fn refused_proposal_answers_restore_the_original_card_before_queued_cards() {
    use magistr_cockpit::cockpit::ProposalKind;
    for kind in ProposalKind::ALL {
        for (accept, click) in [(true, false), (false, false), (true, true), (false, true)] {
            let mut m = model();
            // The record's field for each proposal kind, as the store
            // spells it.
            let (proposal_key, suggestion_key, suggestion) = match kind {
                ProposalKind::Reprioritize => {
                    ("reprioritize", "suggested_priority", serde_json::json!(300))
                }
                ProposalKind::Status => (
                    "status_proposal",
                    "suggested_status",
                    serde_json::json!("done"),
                ),
                ProposalKind::Title => (
                    "title_proposal",
                    "suggested_title",
                    serde_json::json!("updated"),
                ),
            };
            update(
                &mut m,
                frame(
                    "roadmap",
                    "chunk_changed",
                    serde_json::json!({
                        "chunk": {
                            "id": "base",
                            proposal_key: { suggestion_key: suggestion, "reason": "review this" }
                        }
                    }),
                ),
            );
            update(&mut m, blocker_frame("queued"));
            let before: Vec<_> = m.dock().cards().cloned().collect();
            let input = if click {
                left_click(if accept {
                    Widget::Accept.id()
                } else {
                    Widget::Reject.id()
                })
            } else {
                key(KeyCode::Char(if accept { 'y' } else { 'n' }))
            };
            let commands = update(&mut m, input);
            assert_eq!(
                commands,
                vec![Cmd::Engine(EngineCmd::ResolveProposal {
                    chunk: "base".to_owned(),
                    kind,
                    accept,
                })]
            );
            assert_eq!(m.dock().cards().cloned().collect::<Vec<_>>(), before[1..]);
            let error = refused_answer(&mut m, &commands[0]);
            assert_refusal_error(&m, &error);
            assert_eq!(
                m.dock().cards().cloned().collect::<Vec<_>>(),
                before,
                "{kind:?}, accept={accept}, click={click}"
            );
            assert_eq!(
                update(&mut m, key(KeyCode::Char(if accept { 'y' } else { 'n' }))),
                commands,
                "the restored card can be answered again"
            );
            assert_refusal_error(&m, &error);
        }
    }
}

#[test]
fn refused_blocker_answers_restore_the_original_card_before_queued_cards() {
    for click in [false, true] {
        let mut m = model();
        update(&mut m, blocker_frame("base"));
        update(&mut m, proposal_frame("queued"));
        let before: Vec<_> = m.dock().cards().cloned().collect();
        let input = if click {
            left_click(Widget::Clear.id())
        } else {
            key(KeyCode::Char('c'))
        };
        let commands = update(&mut m, input);
        assert_eq!(
            commands,
            vec![Cmd::Engine(EngineCmd::ClearBlocker {
                chunk: "base".to_owned(),
            })]
        );
        assert_eq!(m.dock().cards().cloned().collect::<Vec<_>>(), before[1..]);
        let error = refused_answer(&mut m, &commands[0]);
        assert_refusal_error(&m, &error);
        assert_eq!(
            m.dock().cards().cloned().collect::<Vec<_>>(),
            before,
            "click={click}"
        );
        assert_eq!(update(&mut m, key(KeyCode::Char('c'))), commands);
        assert_refusal_error(&m, &error);
    }
}
