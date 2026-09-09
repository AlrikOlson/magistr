use super::*;

#[test]
fn label_returns_human_readable() {
    assert_eq!(WorkflowPhase::Researching.label(), "Researching...");
    assert_eq!(WorkflowPhase::Complete.label(), "Complete");
}

#[test]
fn step_index_is_ordered() {
    assert_eq!(WorkflowPhase::Researching.step_index(), 0);
    assert_eq!(WorkflowPhase::Planning.step_index(), 1);
    assert_eq!(WorkflowPhase::Executing.step_index(), 2);
    assert_eq!(WorkflowPhase::Verifying.step_index(), 3);
    assert_eq!(WorkflowPhase::Complete.step_index(), 4);
}

#[test]
fn from_name_short_and_participle() {
    assert_eq!(
        WorkflowPhase::from_name("research"),
        Some(WorkflowPhase::Researching)
    );
    assert_eq!(
        WorkflowPhase::from_name("Researching"),
        Some(WorkflowPhase::Researching)
    );
    assert_eq!(
        WorkflowPhase::from_name("plan"),
        Some(WorkflowPhase::Planning)
    );
    assert_eq!(
        WorkflowPhase::from_name("done"),
        Some(WorkflowPhase::Complete)
    );
    assert_eq!(WorkflowPhase::from_name("unknown"), None);
}

#[test]
fn display_matches_label() {
    assert_eq!(format!("{}", WorkflowPhase::Executing), "Executing...");
}
