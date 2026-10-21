use super::*;
use crate::ship::application::{CheckExecution, CheckOutcome, CheckRequest, CheckSettlement};
use crate::ship::domain::{
    action::ActionType, check::CheckType, receipt_stage::ReceiptStage, task::TaskType,
};
use crate::ship::persistence::PersistenceConfig;
use magistr_workflow_contracts::{ReadSource, WriteErrorKind};

fn disk() -> (tempfile::TempDir, Persistence, ShipEngine) {
    let directory = tempfile::tempdir().unwrap();
    let persistence = Persistence::new(&PersistenceConfig {
        enabled: true,
        data_dir: directory.path().into(),
    });
    let mut engine = ShipEngine::new("project".into()).with_persistence(persistence.clone());
    engine
        .set_objective("Objective".into(), vec![], vec![], "scope".into())
        .unwrap();
    engine
        .add_task("verify".into(), "Verify".into(), TaskType::Test, None, None)
        .unwrap();
    engine.start_task("verify").unwrap();
    (directory, persistence, engine)
}

fn outcome() -> CheckOutcome {
    CheckOutcome {
        passed: true,
        verified: true,
        details: "the actual completed command".into(),
        command: Some("exit 0".into()),
        exit_code: Some(0),
        execution: CheckExecution::Exited { code: 0 },
        report: None,
        results: None,
    }
}

fn admit(engine: &mut ShipEngine) -> crate::ship::application::CheckAdmission {
    let target = engine.snapshot().admit_task(None).unwrap();
    let revision = engine.acceptance_revision().unwrap();
    engine
        .admit_receipt(
            target,
            CheckRequest {
                options: Default::default(),
                task_id: None,
                check_type: CheckType::Test,
                name: "test".into(),
                passed: None,
                details: String::new(),
                required: true,
                command: Some("exit 0".into()),
                report: None,
            },
            revision,
            None,
        )
        .unwrap()
        .value
}

#[test]
fn two_writers_keep_both_actions_and_allocate_from_latest_state() {
    let (_directory, persistence, mut first) = disk();
    let mut second = ShipEngine::new("project".into()).with_persistence(persistence.clone());
    let a = first
        .record_action(
            None,
            ActionType::Code,
            "first".into(),
            vec![],
            vec![],
            "done".into(),
            None,
        )
        .unwrap();
    let b = second
        .record_action(
            None,
            ActionType::Code,
            "second".into(),
            vec![],
            vec![],
            "done".into(),
            None,
        )
        .unwrap();
    assert!(matches!(a.source, ReadSource::File(_)));
    assert_ne!(a.value.id, b.value.id);
    let restored = ShipEngine::new("project".into()).with_persistence(persistence);
    assert_eq!(
        restored.tasks[0]
            .actions
            .iter()
            .map(|a| a.description.as_str())
            .collect::<Vec<_>>(),
        vec!["first", "second"]
    );
}

#[test]
fn stale_run_and_reused_task_are_refused_without_retargeting() {
    let (_directory, persistence, mut owner) = disk();
    let mut stale = ShipEngine::new("project".into()).with_persistence(persistence.clone());
    let old = owner.snapshot().run_id;
    owner.reset().unwrap();
    assert!(persistence.path_for("project").is_file());
    owner
        .set_objective("New".into(), vec![], vec![], "scope".into())
        .unwrap();
    owner
        .add_task(
            "verify".into(),
            "New task".into(),
            TaskType::Test,
            None,
            None,
        )
        .unwrap();
    owner.start_task("verify").unwrap();
    assert_ne!(owner.snapshot().run_id, old);
    assert_eq!(
        stale.complete_task("verify", vec![]).unwrap_err().kind(),
        "stale_target"
    );
    let restored = ShipEngine::new("project".into()).with_persistence(persistence);
    assert_eq!(restored.tasks[0].title, "New task");
    assert_eq!(restored.tasks[0].status, TaskStatus::Active);
}

#[test]
fn reused_task_in_the_same_run_has_a_new_instance() {
    let (_directory, persistence, mut owner) = disk();
    owner
        .add_task(
            "planned".into(),
            "Original".into(),
            TaskType::Test,
            None,
            None,
        )
        .unwrap();
    let mut stale = ShipEngine::new("project".into()).with_persistence(persistence.clone());
    owner.remove_task("planned").unwrap();
    owner
        .add_task(
            "planned".into(),
            "Replacement".into(),
            TaskType::Test,
            None,
            None,
        )
        .unwrap();
    assert_eq!(
        stale.remove_task("planned").unwrap_err().kind(),
        "stale_target"
    );
    let restored = ShipEngine::new("project".into()).with_persistence(persistence);
    assert_eq!(restored.tasks[1].title, "Replacement");
}

#[test]
fn invalid_reorder_and_failed_store_leave_live_and_durable_state_unchanged() {
    let (_directory, persistence, mut engine) = disk();
    let before = std::fs::read(persistence.path_for("project")).unwrap();
    let task = engine.tasks[0].clone();
    assert!(engine.reorder_task("verify", Some("missing")).is_err());
    assert_eq!(engine.tasks[0], task);
    assert_eq!(
        std::fs::read(persistence.path_for("project")).unwrap(),
        before
    );
    let lock = persistence.lock_path("project");
    std::fs::remove_file(&lock).unwrap();
    std::fs::create_dir(&lock).unwrap();
    let error = engine.complete_task("verify", vec![]).unwrap_err();
    assert!(matches!(
        error.store_error().unwrap().kind,
        WriteErrorKind::BeforeCommit
    ));
    assert!(engine.observation.current().is_err());
    assert_eq!(engine.tasks[0], task);
    assert_eq!(
        std::fs::read(persistence.path_for("project")).unwrap(),
        before
    );
}

#[test]
fn missing_admission_and_changed_outcome_never_create_another_receipt_or_check() {
    let (_directory, _persistence, mut engine) = disk();
    let admission = admit(&mut engine);
    let mut forged = admission.clone();
    forged.receipt_id = "not-admitted".into();
    assert_eq!(
        engine
            .settle_admitted_check(forged, outcome(), None)
            .unwrap_err()
            .effect(),
        "none"
    );
    assert!(engine.tasks[0].checks.is_empty());
    let settled = engine
        .settle_admitted_check(admission.clone(), outcome(), None)
        .unwrap();
    assert_eq!(settled.receipt().value.id, admission.receipt_id);
    let mut altered = outcome();
    altered.details = "different result".into();
    assert_eq!(
        engine
            .settle_admitted_check(admission.clone(), altered, None)
            .unwrap_err()
            .effect(),
        "none"
    );
    assert_eq!(engine.tasks[0].checks.len(), 1);
    assert_eq!(
        engine
            .receipts
            .get(&admission.receipt_id)
            .unwrap()
            .unwrap()
            .outcome,
        Some(outcome())
    );
}

#[test]
fn recorded_outcome_recovers_after_restart_without_another_check_or_attempt() {
    let (_directory, persistence, mut engine) = disk();
    let admission = admit(&mut engine);
    engine.receipts.fail_next_publication(ReceiptStage::Settled);
    let error = engine
        .settle_admitted_check(admission.clone(), outcome(), None)
        .unwrap_err();
    assert_eq!(error.effect(), "unknown");
    assert_eq!(engine.tasks[0].checks.len(), 1);
    assert_eq!(
        engine
            .receipts
            .get(&admission.receipt_id)
            .unwrap()
            .unwrap()
            .stage,
        ReceiptStage::Applying
    );
    let mut restored = ShipEngine::new("project".into()).with_persistence(persistence);
    let result = restored
        .settle_admitted_check(admission.clone(), outcome(), None)
        .unwrap();
    let CheckSettlement::Applied { check, receipt } = result else {
        panic!("exact admitted task remains");
    };
    assert!(matches!(check.source, ReadSource::File(_)));
    assert!(matches!(receipt.source, ReadSource::File(_)));
    assert_ne!(
        check.source, receipt.source,
        "independent records retain independent witnesses"
    );
    assert_eq!(
        check.value.receipt_id.as_deref(),
        Some(admission.receipt_id.as_str())
    );
    assert_eq!(receipt.value.stage, ReceiptStage::Settled);
    assert_eq!(restored.tasks[0].checks.len(), 1);
    restored
        .settle_admitted_check(admission.clone(), outcome(), None)
        .unwrap();
    assert_eq!(restored.tasks[0].checks.len(), 1);
    assert_eq!(
        restored
            .receipts
            .for_run(&admission.target.run_id)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn partial_application_with_discarded_proof_remains_uncertain() {
    let (_directory, persistence, mut engine) = disk();
    let admission = admit(&mut engine);
    engine.receipts.fail_next_publication(ReceiptStage::Settled);
    assert_eq!(
        engine
            .settle_admitted_check(admission.clone(), outcome(), None)
            .unwrap_err()
            .effect(),
        "unknown"
    );
    engine.reload_from_store().unwrap();
    engine.reset().unwrap();
    engine
        .set_objective("Replacement".into(), vec![], vec![], "scope".into())
        .unwrap();
    engine
        .add_task(
            "verify".into(),
            "Replacement task".into(),
            TaskType::Test,
            None,
            None,
        )
        .unwrap();
    let mut restored = ShipEngine::new("project".into()).with_persistence(persistence);
    assert_eq!(
        restored
            .settle_admitted_check(admission.clone(), outcome(), None)
            .unwrap_err()
            .effect(),
        "unknown"
    );
    assert!(restored.tasks[0].checks.is_empty());
    let receipt = restored
        .receipts
        .get(&admission.receipt_id)
        .unwrap()
        .unwrap();
    assert_eq!(receipt.stage, ReceiptStage::Applying);
    assert_eq!(receipt.outcome, Some(outcome()));
    assert!(receipt.settlement.is_none());
}

#[test]
fn finished_cycles_survive_reselection_and_reused_local_task_ids() {
    let (_directory, persistence, mut engine) = disk();
    engine
        .record_action(
            None,
            ActionType::Code,
            "original action".into(),
            vec!["src/main.rs".into()],
            vec![],
            "finished".into(),
            None,
        )
        .unwrap();
    let admission = admit(&mut engine);
    engine
        .settle_admitted_check(admission.clone(), outcome(), None)
        .unwrap();
    engine.complete_task("verify", vec![]).unwrap();
    engine.finalize(vec![], Some("finished".into())).unwrap();
    let original = engine.persisted_state();
    for scope in ["scope", "another"] {
        engine
            .set_objective("Next".into(), vec![], vec![], scope.into())
            .unwrap();
        engine
            .add_task(
                "verify".into(),
                "New task".into(),
                TaskType::Test,
                None,
                None,
            )
            .unwrap();
        engine.start_task("verify").unwrap();
        engine.complete_task("verify", vec![]).unwrap();
        engine.finalize(vec![], None).unwrap();
    }
    let restored = ShipEngine::new("project".into()).with_persistence(persistence);
    assert_eq!(restored.saved_cycles.len(), 2);
    let retained = &restored.saved_cycles[0];
    assert_eq!(retained.objective, original.objective.unwrap());
    assert_eq!(retained.tasks, original.tasks);
    assert_eq!(retained.execution.run_id, original.execution.run_id);
    assert_eq!(retained.execution.tasks, original.execution.tasks);
    assert_ne!(retained.execution.run_id, restored.execution.run_id);
    assert_ne!(
        retained.execution.tasks["verify"],
        restored.execution.tasks["verify"]
    );
    assert_eq!(
        restored
            .receipts
            .get(&admission.receipt_id)
            .unwrap()
            .unwrap()
            .outcome,
        Some(outcome())
    );
}

#[test]
fn switching_back_resumes_only_unfinished_work_and_keeps_finished_history() {
    let (_directory, _persistence, mut engine) = disk();
    let unfinished = engine.execution.run_id.clone();
    engine
        .set_objective("Other".into(), vec![], vec![], "other".into())
        .unwrap();
    engine.finalize(vec![], None).unwrap();
    let finished = engine.execution.run_id.clone();
    engine
        .set_objective("Ignored replacement".into(), vec![], vec![], "scope".into())
        .unwrap();
    assert_eq!(engine.execution.run_id, unfinished);
    assert_eq!(engine.tasks[0].status, TaskStatus::Active);
    assert_eq!(engine.saved_cycles.len(), 1);
    assert_eq!(engine.saved_cycles[0].execution.run_id, finished);
    engine
        .set_objective("New other".into(), vec![], vec![], "other".into())
        .unwrap();
    assert_ne!(engine.execution.run_id, finished);
    assert_eq!(engine.saved_cycles.len(), 2);
}

#[test]
fn finished_observations_keep_lanes_and_receipt_references_separate() {
    let (_directory, persistence, mut engine) = disk();
    let admission = admit(&mut engine);
    engine
        .settle_admitted_check(admission.clone(), outcome(), None)
        .unwrap();
    engine.complete_task("verify", vec![]).unwrap();
    engine.finalize(vec![], None).unwrap();
    let old = engine.observe().unwrap().value.current.unwrap();
    engine
        .set_objective("Next".into(), vec![], vec![], "next".into())
        .unwrap();
    let second_store = persistence.clone().for_execution("another");
    let mut other = ShipEngine::new("project".into()).with_persistence(second_store);
    other
        .set_objective("Another lane".into(), vec![], vec![], "scope".into())
        .unwrap();
    let observed = engine.observe().unwrap();
    assert_eq!(observed.value.run(&old.run_id), Some(&old));
    assert_eq!(observed.value.retained.len(), 1);
    assert!(other.observe().unwrap().value.retained.is_empty());
    assert!(other.receipts.get(&admission.receipt_id).unwrap().is_none());
    engine.compact_receipts(|| Ok(Default::default())).unwrap();
    assert!(
        engine
            .receipts
            .get(&admission.receipt_id)
            .unwrap()
            .is_some()
    );
    let path = persistence.path_for("project");
    let bytes = std::fs::read(&path).unwrap();
    let mut invalid: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    invalid["saved_cycles"][0]["execution"]["run_id"] = invalid["execution"]["run_id"].clone();
    std::fs::write(&path, serde_json::to_vec(&invalid).unwrap()).unwrap();
    assert_eq!(
        engine.observe().unwrap_err().kind,
        magistr_workflow_contracts::ReadErrorKind::Identity
    );
    std::fs::write(&path, bytes).unwrap();
    assert_eq!(engine.observe().unwrap().value, observed.value);
}
