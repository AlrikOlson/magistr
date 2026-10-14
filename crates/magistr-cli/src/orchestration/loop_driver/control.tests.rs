use super::*;
use magistr_domain::{
    identity::{ProjectId, RunId},
    workflow::RunMode,
};

fn fixture() -> (Control, tokio::sync::mpsc::UnboundedReceiver<Msg>) {
    let (inbox, events) = Inbox::channel();
    (
        Control {
            inbox,
            state: Arc::new(Mutex::new(WorkRun {
                id: RunId::new(ProjectId::new("project").unwrap(), "one").unwrap(),
                mode: RunMode::ChunkLoop,
                track: None,
                chunk: Some("first".into()),
                iteration: 1,
                limit: 10,
                phase: WorkPhase::Running,
                requested: None,
            })),
        },
        events,
    )
}
fn request(control: &Control, action: LoopCmd) -> Result<(), String> {
    control.request(RunCommand {
        run_id: control.snapshot().id,
        action,
    })
}
#[test]
fn pause_is_requested_during_work_and_only_becomes_paused_at_the_boundary() {
    let (control, mut events) = fixture();
    request(&control, LoopCmd::Pause).unwrap();
    assert_eq!(control.snapshot().phase, WorkPhase::Running);
    assert_eq!(control.snapshot().requested, Some(LoopCmd::Pause));
    request(&control, LoopCmd::Pause).unwrap();
    assert!(matches!(events.try_recv().unwrap(), Msg::WorkRun(_)));
    assert!(
        events.try_recv().is_err(),
        "repeated requests do not invent a transition"
    );
    control.boundary();
    assert_eq!(control.snapshot().phase, WorkPhase::Paused);
    assert_eq!(control.snapshot().requested, None);
}
#[test]
fn step_runs_the_current_or_next_item_then_pauses_and_resume_can_clear_a_pause() {
    let (control, _) = fixture();
    request(&control, LoopCmd::Step).unwrap();
    control.boundary();
    assert_eq!(control.snapshot().phase, WorkPhase::Paused);
    request(&control, LoopCmd::Step).unwrap();
    request(&control, LoopCmd::Step).unwrap();
    control.boundary();
    assert_eq!(
        control.snapshot().phase,
        WorkPhase::Starting,
        "one new item is admitted"
    );
    control.update(|state| state.phase = WorkPhase::Running);
    control.boundary();
    assert_eq!(control.snapshot().phase, WorkPhase::Paused);
    request(&control, LoopCmd::Resume).unwrap();
    assert_eq!(control.snapshot().phase, WorkPhase::Starting);
    assert_eq!(control.snapshot().requested, None);
}
#[test]
fn stop_is_sticky_and_stale_or_finished_controls_do_not_change_work() {
    let (control, mut events) = fixture();
    request(&control, LoopCmd::Abort).unwrap();
    for action in [
        LoopCmd::Resume,
        LoopCmd::Step,
        LoopCmd::Pause,
        LoopCmd::Skip,
    ] {
        assert!(request(&control, action).is_err());
        assert_eq!(control.snapshot().requested, Some(LoopCmd::Abort));
    }
    request(&control, LoopCmd::Abort).unwrap();
    let before = control.snapshot();
    let wrong = RunCommand {
        run_id: RunId::new(ProjectId::new("project").unwrap(), "another").unwrap(),
        action: LoopCmd::Resume,
    };
    assert!(control.request(wrong).is_err());
    assert_eq!(control.snapshot(), before);
    assert!(events.try_recv().is_ok());
    assert!(events.try_recv().is_err());
    control.end("Stopped after the current item.".into());
    assert!(request(&control, LoopCmd::Resume).is_err());
    assert!(!control.snapshot().is_active());
}
