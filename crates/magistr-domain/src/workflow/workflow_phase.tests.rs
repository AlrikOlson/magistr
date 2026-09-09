use alloc::string::ToString;

use super::*;

// ── WorkflowPhase tests ─────────────────────────────────────────────

#[test]
fn phase_labels() {
    assert_eq!(WorkflowPhase::Researching.label(), "Researching");
    assert_eq!(WorkflowPhase::Planning.label(), "Planning");
    assert_eq!(WorkflowPhase::Executing.label(), "Executing");
    assert_eq!(WorkflowPhase::Verifying.label(), "Verifying");
    assert_eq!(WorkflowPhase::Complete.label(), "Complete");
}

#[test]
fn phase_step_indices() {
    assert_eq!(WorkflowPhase::Researching.step_index(), 0);
    assert_eq!(WorkflowPhase::Planning.step_index(), 1);
    assert_eq!(WorkflowPhase::Executing.step_index(), 2);
    assert_eq!(WorkflowPhase::Verifying.step_index(), 3);
    assert_eq!(WorkflowPhase::Complete.step_index(), 4);
}

#[test]
fn phase_from_name() {
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
        WorkflowPhase::from_name("execute"),
        Some(WorkflowPhase::Executing)
    );
    assert_eq!(
        WorkflowPhase::from_name("verify"),
        Some(WorkflowPhase::Verifying)
    );
    assert_eq!(
        WorkflowPhase::from_name("done"),
        Some(WorkflowPhase::Complete)
    );
    assert_eq!(WorkflowPhase::from_name("unknown"), None);
}

#[test]
fn phase_display() {
    assert_eq!(WorkflowPhase::Researching.to_string(), "Researching");
    assert_eq!(WorkflowPhase::Complete.to_string(), "Complete");
}

#[test]
fn phase_is_terminal() {
    assert!(WorkflowPhase::Complete.is_terminal());
    assert!(!WorkflowPhase::Researching.is_terminal());
    assert!(!WorkflowPhase::Verifying.is_terminal());
}

#[test]
fn phase_all_returns_ordered() {
    let all = WorkflowPhase::all();
    assert_eq!(all.len(), 5);
    for (i, phase) in all.iter().enumerate() {
        assert_eq!(phase.step_index(), i);
    }
}

#[test]
fn phase_serialization_roundtrip() {
    let phase = WorkflowPhase::Executing;
    let json = serde_json::to_string(&phase).unwrap();
    assert_eq!(json, "\"executing\"");
    let deserialized: WorkflowPhase = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, phase);
}

#[test]
fn phase_transitions_preserve_workflow_order_and_terminal_restart() {
    let allowed = [
        [true, true, true, true, true],
        [false, true, true, true, true],
        [false, false, true, true, true],
        [false, false, false, true, true],
        [true, true, true, true, true],
    ];
    for (from_index, &from) in WorkflowPhase::all().iter().enumerate() {
        for (to_index, &to) in WorkflowPhase::all().iter().enumerate() {
            assert_eq!(
                from.can_advance_to(to),
                allowed[from_index][to_index],
                "transition from {from} to {to}"
            );
        }
    }
}
