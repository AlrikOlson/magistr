use super::*;

#[test]
fn dispatch_phase_survives_interruptions_and_handle_attachment_is_monotonic() {
    let mut ledger = OperationLedger::new();
    let pending = accepted(&mut ledger, "pending", OperationKind::ProviderDispatch);
    let started = accepted(&mut ledger, "started", OperationKind::ProviderDispatch);
    ledger.start(&started, None).unwrap();
    ledger.start(&started, Some("pid:42".into())).unwrap();
    assert!(ledger.start(&started, Some("pid:43".into())).is_err());
    ledger.mark_interrupted();
    ledger.mark_interrupted();
    assert_eq!(
        ledger.record(&pending).unwrap().dispatch_phase,
        Some(DispatchPhase::Pending)
    );
    assert_eq!(
        ledger.record(&started).unwrap().dispatch_phase,
        Some(DispatchPhase::Started)
    );
    let restored: OperationLedger =
        serde_json::from_str(&serde_json::to_string(&ledger).unwrap()).unwrap();
    assert_eq!(restored, ledger);
    assert!(ledger.start(&started, Some("pid:42".into())).is_err());
}

#[test]
fn an_operator_resolves_only_one_open_dispatch_and_keeps_the_explanation() {
    for outcome in [
        OperationOutcome::Completed,
        OperationOutcome::Failed,
        OperationOutcome::Cancelled,
    ] {
        let mut ledger = OperationLedger::new();
        let dispatch = accepted(&mut ledger, "dispatch", OperationKind::ProviderDispatch);
        let other = accepted(&mut ledger, "receipt", OperationKind::Receipt);
        assert!(ledger.resolve_dispatch(&other, outcome, "checked").is_err());
        assert!(
            ledger
                .resolve_dispatch(&dispatch, OperationOutcome::Running, "checked")
                .is_err()
        );
        assert!(ledger.resolve_dispatch(&dispatch, outcome, "  ").is_err());
        ledger.start(&dispatch, None).unwrap();
        ledger.mark_interrupted();
        ledger
            .resolve_dispatch(&dispatch, outcome, " Checked the stopped agent. ")
            .unwrap();
        assert_eq!(
            ledger.record(&dispatch).unwrap().resolution.as_deref(),
            Some("Checked the stopped agent.")
        );
        assert_eq!(
            ledger.record(&other).unwrap().outcome,
            OperationOutcome::Indeterminate
        );
        assert!(
            ledger
                .resolve_dispatch(&dispatch, outcome, "again")
                .is_err()
        );
        if outcome != OperationOutcome::Completed {
            assert_eq!(
                accepted(&mut ledger, "dispatch", OperationKind::ProviderDispatch)
                    .number()
                    .get(),
                2
            );
        } else {
            assert!(matches!(
                ledger
                    .admit(
                        command("dispatch"),
                        OperationKind::ProviderDispatch,
                        "target"
                    )
                    .unwrap(),
                OperationAdmission::Established(_)
            ));
        }
    }
}

#[test]
fn impossible_dispatch_launch_evidence_is_rejected() {
    let mut ledger = OperationLedger::new();
    let dispatch = accepted(&mut ledger, "dispatch", OperationKind::ProviderDispatch);
    ledger.record_mut(&dispatch).unwrap().external = Some("pid:42".into());
    assert!(ledger.validate().is_err());
}

#[test]
fn legacy_dispatches_have_unknown_launch_phase() {
    let mut ledger = OperationLedger::new();
    let attempt = accepted(&mut ledger, "legacy", OperationKind::ProviderDispatch);
    let mut json = serde_json::to_value(ledger).unwrap();
    json["records"][0]
        .as_object_mut()
        .unwrap()
        .remove("dispatch_phase");
    let mut restored: OperationLedger = serde_json::from_value(json).unwrap();
    restored.mark_interrupted();
    assert_eq!(restored.record(&attempt).unwrap().dispatch_phase, None);
}

use alloc::vec;
use alloc::vec::Vec;

use crate::identity::{ProjectId, RunId};

fn command(key: &str) -> CommandId {
    let run = RunId::new(ProjectId::new("proj").unwrap(), "run-1").unwrap();
    CommandId::new(run, key).unwrap()
}

fn attempt(key: &str, number: u32) -> AttemptId {
    AttemptId::new(command(key), NonZeroU32::new(number).unwrap())
}

fn accepted(ledger: &mut OperationLedger, key: &str, kind: OperationKind) -> AttemptId {
    match ledger.admit(command(key), kind, "target").unwrap() {
        OperationAdmission::Accepted(attempt) => attempt,
        OperationAdmission::Established(record) => panic!("already established: {record:?}"),
    }
}

#[test]
fn a_new_command_is_admitted_as_attempt_one_and_written_accepted() {
    let mut ledger = OperationLedger::new();
    let attempt = accepted(&mut ledger, "dispatch", OperationKind::ProviderDispatch);
    assert_eq!(attempt, attempt_of("dispatch", 1));
    let record = ledger.record(&attempt).unwrap();
    assert_eq!(record.outcome, OperationOutcome::Accepted);
    assert_eq!(record.kind, OperationKind::ProviderDispatch);
    assert_eq!(record.target, "target");
    assert_eq!(record.external, None);
    assert_eq!(ledger.unsettled().count(), 1);
}

fn attempt_of(key: &str, number: u32) -> AttemptId {
    attempt(key, number)
}

#[test]
fn only_a_completed_outcome_deduplicates() {
    let mut ledger = OperationLedger::new();
    let first = accepted(&mut ledger, "complete", OperationKind::NativeMutation);
    ledger.settle(&first, OperationOutcome::Completed).unwrap();
    let again = ledger
        .admit(command("complete"), OperationKind::NativeMutation, "target")
        .unwrap();
    assert_eq!(again.accepted(), None);
    assert!(matches!(again, OperationAdmission::Established(ref r) if r.attempt == first));
    assert_eq!(ledger.records().len(), 1);
}

#[test]
fn failed_and_cancelled_admit_the_next_attempt_number() {
    let mut ledger = OperationLedger::new();
    let first = accepted(&mut ledger, "gate", OperationKind::Receipt);
    ledger.settle(&first, OperationOutcome::Failed).unwrap();
    let second = accepted(&mut ledger, "gate", OperationKind::Receipt);
    assert_eq!(second, attempt("gate", 2));
    ledger.settle(&second, OperationOutcome::Cancelled).unwrap();
    let third = accepted(&mut ledger, "gate", OperationKind::Receipt);
    assert_eq!(third, attempt("gate", 3));
    assert_eq!(ledger.records().len(), 3);
    assert!(ledger.record(&first).unwrap().is_settled());
}

#[test]
fn an_unsettled_attempt_refuses_another_admission_until_reconciled() {
    let mut ledger = OperationLedger::new();
    let first = accepted(&mut ledger, "dispatch", OperationKind::ProviderDispatch);
    for outcome in [
        OperationOutcome::Accepted,
        OperationOutcome::Running,
        OperationOutcome::Indeterminate,
    ] {
        let refused = ledger
            .admit(
                command("dispatch"),
                OperationKind::ProviderDispatch,
                "target",
            )
            .unwrap_err();
        assert!(
            matches!(refused, OperationError::Unsettled(ref r) if r.attempt == first && r.outcome == outcome),
            "{outcome}: {refused}"
        );
        match outcome {
            OperationOutcome::Accepted => ledger.start(&first, Some("pid:42".into())).unwrap(),
            OperationOutcome::Running => {
                assert_eq!(ledger.mark_interrupted(), vec![first.clone()]);
            }
            _ => {}
        }
    }
    assert_eq!(
        ledger.record(&first).unwrap().external.as_deref(),
        Some("pid:42")
    );
    ledger.reconcile(&first, OperationOutcome::Failed).unwrap();
    assert_eq!(
        accepted(&mut ledger, "dispatch", OperationKind::ProviderDispatch),
        attempt("dispatch", 2)
    );
}

#[test]
fn a_command_id_cannot_name_a_different_operation() {
    let mut ledger = OperationLedger::new();
    let first = accepted(&mut ledger, "op", OperationKind::Receipt);
    ledger.settle(&first, OperationOutcome::Completed).unwrap();
    let other_kind = ledger
        .admit(command("op"), OperationKind::Publication, "target")
        .unwrap_err();
    assert!(matches!(other_kind, OperationError::Conflict(_)));
    let other_target = ledger
        .admit(command("op"), OperationKind::Receipt, "elsewhere")
        .unwrap_err();
    assert!(matches!(other_target, OperationError::Conflict(_)));
}

#[test]
fn settled_outcomes_latch_and_repeats_are_idempotent() {
    let mut ledger = OperationLedger::new();
    let attempt = accepted(&mut ledger, "op", OperationKind::NativeMutation);
    ledger.start(&attempt, None).unwrap();
    ledger
        .settle(&attempt, OperationOutcome::Completed)
        .unwrap();
    ledger
        .settle(&attempt, OperationOutcome::Completed)
        .unwrap();
    let refused = ledger
        .settle(&attempt, OperationOutcome::Failed)
        .unwrap_err();
    assert_eq!(
        refused,
        OperationError::Transition {
            attempt: attempt.clone(),
            from: OperationOutcome::Completed,
            to: OperationOutcome::Failed,
        }
    );
    assert!(ledger.start(&attempt, None).is_err());
    assert!(
        ledger
            .reconcile(&attempt, OperationOutcome::Failed)
            .is_err()
    );
    assert_eq!(ledger.mark_interrupted(), Vec::<AttemptId>::new());
}

#[test]
fn a_live_owner_may_report_a_lost_response_but_only_recovery_resolves_it() {
    let mut ledger = OperationLedger::new();
    let attempt = accepted(&mut ledger, "op", OperationKind::ProviderDispatch);
    ledger.start(&attempt, Some("pid:7".into())).unwrap();
    ledger
        .settle(&attempt, OperationOutcome::Indeterminate)
        .unwrap();
    assert!(
        ledger
            .settle(&attempt, OperationOutcome::Completed)
            .is_err()
    );
    assert_eq!(ledger.indeterminate().count(), 1);
    assert!(
        ledger
            .reconcile(&attempt, OperationOutcome::Running)
            .is_err()
    );
    ledger
        .reconcile(&attempt, OperationOutcome::Completed)
        .unwrap();
    assert_eq!(ledger.unsettled().count(), 0);
}

#[test]
fn unknown_attempts_are_refused_by_name() {
    let mut ledger = OperationLedger::new();
    let ghost = attempt("ghost", 1);
    assert_eq!(
        ledger.start(&ghost, None).unwrap_err(),
        OperationError::Unknown(ghost.clone())
    );
    assert!(ledger.settle(&ghost, OperationOutcome::Failed).is_err());
    assert!(ledger.reconcile(&ghost, OperationOutcome::Failed).is_err());
    assert_eq!(ledger.record(&ghost), None);
}

#[test]
fn restart_marks_every_in_flight_attempt_indeterminate_and_nothing_else() {
    let mut ledger = OperationLedger::new();
    let reserved = accepted(&mut ledger, "reserve", OperationKind::Reservation);
    ledger
        .settle(&reserved, OperationOutcome::Completed)
        .unwrap();
    let accepted_only = accepted(&mut ledger, "frame", OperationKind::Publication);
    let running = accepted(&mut ledger, "dispatch", OperationKind::ProviderDispatch);
    ledger.start(&running, Some("pid:9".into())).unwrap();
    let marked = ledger.mark_interrupted();
    assert_eq!(marked, vec![accepted_only.clone(), running.clone()]);
    assert_eq!(ledger.indeterminate().count(), 2);
    assert_eq!(
        ledger.record(&reserved).unwrap().outcome,
        OperationOutcome::Completed
    );
    assert_eq!(
        ledger.record(&running).unwrap().external.as_deref(),
        Some("pid:9")
    );
}

#[test]
fn persisted_ledgers_are_validated_before_installation() {
    let mut ledger = OperationLedger::new();
    let first = accepted(&mut ledger, "op", OperationKind::Receipt);
    ledger.settle(&first, OperationOutcome::Failed).unwrap();
    let second = accepted(&mut ledger, "op", OperationKind::Receipt);
    assert_eq!(ledger.validate(), Ok(()));
    let json = serde_json::to_string(&ledger).unwrap();
    let restored: OperationLedger = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, ledger);

    let mut twice = ledger.clone();
    twice.records.push(twice.record(&second).unwrap().clone());
    assert_eq!(twice.validate(), Err("operation attempt recorded twice"));

    let mut two_open = ledger.clone();
    two_open.records.push(OperationRecord {
        attempt: attempt("op", 3),
        kind: OperationKind::Receipt,
        target: "target".into(),
        outcome: OperationOutcome::Running,
        external: None,
        dispatch_phase: None,
        resolution: None,
    });
    assert_eq!(
        two_open.validate(),
        Err("a command has two unsettled attempts")
    );

    let mut renamed = ledger.clone();
    renamed.records.push(OperationRecord {
        attempt: attempt("op", 3),
        kind: OperationKind::Publication,
        target: "target".into(),
        outcome: OperationOutcome::Completed,
        external: None,
        dispatch_phase: None,
        resolution: None,
    });
    assert_eq!(
        renamed.validate(),
        Err("one command id names two different operations")
    );

    let missing_external = r#"{"records":[{"attempt":{"command":{"run":{"project":"proj","local":"run-1"},"local":"op"},"number":1},"kind":"receipt","target":"t","outcome":"accepted"}]}"#;
    assert!(serde_json::from_str::<OperationLedger>(missing_external).is_err());
}
