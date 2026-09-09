use super::*;

#[test]
fn phase_labels_are_nonempty() {
    let phases = [
        Phase::Detecting,
        Phase::LoadingPreset,
        Phase::Wizard,
        Phase::Scaffolding,
        Phase::Connecting,
        Phase::Running,
        Phase::AwaitingInput,
        Phase::Cancelling,
        Phase::Done,
        Phase::Error("test".into()),
        Phase::Roadmap,
    ];
    for phase in &phases {
        assert!(
            !phase.label().is_empty(),
            "Phase {:?} has empty label",
            phase
        );
    }
}

#[test]
fn terminal_phases() {
    assert!(Phase::Done.is_terminal());
    assert!(Phase::Error("boom".into()).is_terminal());
    assert!(!Phase::Running.is_terminal());
    assert!(!Phase::Detecting.is_terminal());
}
