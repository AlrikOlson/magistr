use magistr_engine::native::WaitingReason;
use magistr_engine::{
    Delivery, NativeConfig, NativeEngines, NativeEvent, PlanFrame, ProjectAuthority, TaskStatus,
    WriterRole,
};

use super::*;
use magistr_cockpit::cockpit::EngineCmd;
use magistr_cockpit::engine::{EngineEvent, apply};

/// In-memory engines on a throwaway project directory, with one pending
/// task the dock could raise a card for.
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
        .plan_add_task(
            "auth".to_owned(),
            "Authenticate users".to_owned(),
            String::new(),
            TaskStatus::ToDo,
            200,
            String::new(),
            vec![],
            vec![],
            None,
            None,
        )
        .expect("the task");
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
fn an_accepted_reprioritize_moves_the_task_and_the_frame_takes_the_card() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let engines = engines(&tmp);
    engines
        .plan_reprioritize("auth", 100, "blocking the release".to_owned())
        .expect("the proposal");

    // The proposal is a card on the dock, the way the feed's frame puts
    // it there.

    let mut port = NativeEnginePort::new(engines.clone());
    let frame = frame_of(
        &mut port,
        &EngineCmd::ResolveProposal {
            task: "auth".to_owned(),
            kind: ProposalKind::Reprioritize,
            accept: true,
        },
    );

    // The store moved…
    let after = engines.plan_task("auth").unwrap().expect("the task");
    assert_eq!(after.priority, 100, "the suggestion was applied");
    assert!(after.reprioritize.is_none(), "and the proposal is disposed");
    // …and the event the answer came back as is what takes the card.
    assert!(matches!(
        &frame,
        NativeEvent::Plan(PlanFrame::TaskChanged { task }) if task.id == "auth"
    ));
}

#[test]
fn a_rejected_reprioritize_disposes_the_proposal_and_leaves_the_priority() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let engines = engines(&tmp);
    engines
        .plan_reprioritize("auth", 100, "blocking the release".to_owned())
        .expect("the proposal");

    let mut port = NativeEnginePort::new(engines.clone());
    let frame = frame_of(
        &mut port,
        &EngineCmd::ResolveProposal {
            task: "auth".to_owned(),
            kind: ProposalKind::Reprioritize,
            accept: false,
        },
    );
    let after = engines.plan_task("auth").unwrap().expect("the task");
    assert_eq!(after.priority, 200, "a rejection applies nothing");
    assert!(after.reprioritize.is_none(), "and clears the proposal");
    assert!(matches!(
        frame,
        NativeEvent::Plan(PlanFrame::TaskChanged { task }) if task.reprioritize.is_none()
    ));
}

#[test]
fn a_cleared_blocker_lifts_waiting_on_and_the_frame_takes_the_card() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let engines = engines(&tmp);
    engines
        .plan_set_waiting_on(
            "auth",
            WaitingReason::Person,
            "needs a person to pick the scheme".to_owned(),
            None,
        )
        .expect("set");

    let mut port = NativeEnginePort::new(engines.clone());
    let frame = frame_of(
        &mut port,
        &EngineCmd::ClearBlocker {
            task: "auth".to_owned(),
        },
    );
    assert!(
        engines
            .plan_task("auth")
            .unwrap()
            .expect("the task")
            .waiting_on
            .is_none(),
        "the blocker is lifted in the store"
    );

    assert!(
        matches!(frame, NativeEvent::Plan(PlanFrame::TaskChanged {task}) if task.waiting_on.is_none())
    );
    // The engines refuse a second clear rather than answer a no-op, and
    // the cockpit hears it as words for the stage.
    assert!(matches!(
        apply(
            &mut port,
            &EngineCmd::ClearBlocker {
                task: "auth".to_owned(),
            }
        ),
        EngineEvent::PortFailed {
            cmd: EngineCmd::ClearBlocker { task },
            ..
        } if task == "auth"
    ));
}

/// The event the external server announces for a changed task — what
/// the dock folds when the feed, rather than the port, reports.

#[test]
fn a_missing_decision_is_refused_with_its_original_command() {
    let temp = tempfile::tempdir().unwrap();
    let engines = engines(&temp);
    let mut port = NativeEnginePort::new(engines);
    for kind in ProposalKind::ALL {
        for accept in [true, false] {
            let cmd = EngineCmd::ResolveProposal {
                task: "missing".into(),
                kind,
                accept,
            };
            assert!(
                matches!(apply(&mut port,&cmd),EngineEvent::PortFailed {cmd:ref original,error,..} if original==&cmd && !error.is_empty())
            );
        }
    }
    let cmd = EngineCmd::ClearBlocker {
        task: "missing".into(),
    };
    assert!(
        matches!(apply(&mut port,&cmd),EngineEvent::PortFailed {cmd:ref original,error,..} if original==&cmd && !error.is_empty())
    );
}
