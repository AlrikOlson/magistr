use super::*;
use crate::run::application::{CheckExecution, CheckOutcome, CheckRequest, CheckSettlement};
use crate::run::domain::{
    check::CheckType, check_result_stage::CheckResultStage, progress::ProgressType, step::StepType,
};
use crate::run::persistence::PersistenceConfig;
use magistr_workflow_contracts::{ReadSource, WriteErrorKind};

fn disk() -> (tempfile::TempDir, Persistence, RunEngine) {
    let directory = tempfile::tempdir().unwrap();
    let persistence = Persistence::new(&PersistenceConfig {
        enabled: true,
        data_dir: directory.path().into(),
    });
    let mut engine = RunEngine::new("project".into()).with_persistence(persistence.clone());
    engine
        .set_goal("Goal".into(), vec![], vec![], "scope".into())
        .unwrap();
    engine
        .add_step("verify".into(), "Verify".into(), StepType::Test, None, None)
        .unwrap();
    engine.start_step("verify").unwrap();
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

fn admit(engine: &mut RunEngine) -> crate::run::application::CheckAdmission {
    let target = engine.snapshot().admit_step(None).unwrap();
    let revision = engine.acceptance_revision().unwrap();
    engine
        .admit_check_result(
            target,
            CheckRequest {
                options: Default::default(),
                step_id: None,
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
fn two_writers_keep_both_progress_and_allocate_from_latest_state() {
    let (_directory, persistence, mut first) = disk();
    let mut second = RunEngine::new("project".into()).with_persistence(persistence.clone());
    let a = first
        .record_progress(
            None,
            ProgressType::Code,
            "first".into(),
            vec![],
            vec![],
            "done".into(),
            None,
        )
        .unwrap();
    let b = second
        .record_progress(
            None,
            ProgressType::Code,
            "second".into(),
            vec![],
            vec![],
            "done".into(),
            None,
        )
        .unwrap();
    assert!(matches!(a.source, ReadSource::File(_)));
    assert_ne!(a.value.id, b.value.id);
    let restored = RunEngine::new("project".into()).with_persistence(persistence);
    assert_eq!(
        restored.steps[0]
            .progress
            .iter()
            .map(|a| a.description.as_str())
            .collect::<Vec<_>>(),
        vec!["first", "second"]
    );
}

#[test]
fn stale_run_and_reused_step_are_refused_without_retargeting() {
    let (_directory, persistence, mut owner) = disk();
    let mut stale = RunEngine::new("project".into()).with_persistence(persistence.clone());
    let old = owner.snapshot().run_id;
    owner.reset().unwrap();
    assert!(persistence.path_for("project").is_file());
    owner
        .set_goal("New".into(), vec![], vec![], "scope".into())
        .unwrap();
    owner
        .add_step(
            "verify".into(),
            "New step".into(),
            StepType::Test,
            None,
            None,
        )
        .unwrap();
    owner.start_step("verify").unwrap();
    assert_ne!(owner.snapshot().run_id, old);
    assert_eq!(
        stale.complete_step("verify", vec![]).unwrap_err().kind(),
        "stale_target"
    );
    let restored = RunEngine::new("project".into()).with_persistence(persistence);
    assert_eq!(restored.steps[0].title, "New step");
    assert_eq!(restored.steps[0].status, StepStatus::Active);
}

#[test]
fn reused_step_in_the_same_run_has_a_new_instance() {
    let (_directory, persistence, mut owner) = disk();
    owner
        .add_step(
            "planned".into(),
            "Original".into(),
            StepType::Test,
            None,
            None,
        )
        .unwrap();
    let mut stale = RunEngine::new("project".into()).with_persistence(persistence.clone());
    owner.remove_step("planned").unwrap();
    owner
        .add_step(
            "planned".into(),
            "Replacement".into(),
            StepType::Test,
            None,
            None,
        )
        .unwrap();
    assert_eq!(
        stale.remove_step("planned").unwrap_err().kind(),
        "stale_target"
    );
    let restored = RunEngine::new("project".into()).with_persistence(persistence);
    assert_eq!(restored.steps[1].title, "Replacement");
}

#[test]
fn invalid_reorder_and_failed_store_leave_live_and_durable_state_unchanged() {
    let (_directory, persistence, mut engine) = disk();
    let before = std::fs::read(persistence.path_for("project")).unwrap();
    let step = engine.steps[0].clone();
    assert!(engine.reorder_step("verify", Some("missing")).is_err());
    assert_eq!(engine.steps[0], step);
    assert_eq!(
        std::fs::read(persistence.path_for("project")).unwrap(),
        before
    );
    let lock = persistence.lock_path("project");
    std::fs::remove_file(&lock).unwrap();
    std::fs::create_dir(&lock).unwrap();
    let error = engine.complete_step("verify", vec![]).unwrap_err();
    assert!(matches!(
        error.store_error().unwrap().kind,
        WriteErrorKind::BeforeCommit
    ));
    assert!(engine.observation.current().is_err());
    assert_eq!(engine.steps[0], step);
    assert_eq!(
        std::fs::read(persistence.path_for("project")).unwrap(),
        before
    );
}

#[test]
fn missing_admission_and_changed_outcome_never_create_another_check_result_or_check() {
    let (_directory, _persistence, mut engine) = disk();
    let admission = admit(&mut engine);
    let mut forged = admission.clone();
    forged.check_result_id = "not-admitted".into();
    assert_eq!(
        engine
            .settle_admitted_check(forged, outcome(), None)
            .unwrap_err()
            .effect(),
        "none"
    );
    assert!(engine.steps[0].checks.is_empty());
    let settled = engine
        .settle_admitted_check(admission.clone(), outcome(), None)
        .unwrap();
    assert_eq!(settled.check_result().value.id, admission.check_result_id);
    let mut altered = outcome();
    altered.details = "different result".into();
    assert_eq!(
        engine
            .settle_admitted_check(admission.clone(), altered, None)
            .unwrap_err()
            .effect(),
        "none"
    );
    assert_eq!(engine.steps[0].checks.len(), 1);
    assert_eq!(
        engine
            .check_results
            .get(&admission.check_result_id)
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
    engine
        .check_results
        .fail_next_publication(CheckResultStage::Settled);
    let error = engine
        .settle_admitted_check(admission.clone(), outcome(), None)
        .unwrap_err();
    assert_eq!(error.effect(), "unknown");
    assert_eq!(engine.steps[0].checks.len(), 1);
    assert_eq!(
        engine
            .check_results
            .get(&admission.check_result_id)
            .unwrap()
            .unwrap()
            .stage,
        CheckResultStage::Applying
    );
    let mut restored = RunEngine::new("project".into()).with_persistence(persistence);
    let result = restored
        .settle_admitted_check(admission.clone(), outcome(), None)
        .unwrap();
    let CheckSettlement::Applied {
        check,
        check_result,
    } = result
    else {
        panic!("exact admitted step remains");
    };
    assert!(matches!(check.source, ReadSource::File(_)));
    assert!(matches!(check_result.source, ReadSource::File(_)));
    assert_ne!(
        check.source, check_result.source,
        "independent records retain independent witnesses"
    );
    assert_eq!(
        check.value.check_result_id.as_deref(),
        Some(admission.check_result_id.as_str())
    );
    assert_eq!(check_result.value.stage, CheckResultStage::Settled);
    assert_eq!(restored.steps[0].checks.len(), 1);
    restored
        .settle_admitted_check(admission.clone(), outcome(), None)
        .unwrap();
    assert_eq!(restored.steps[0].checks.len(), 1);
    assert_eq!(
        restored
            .check_results
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
    engine
        .check_results
        .fail_next_publication(CheckResultStage::Settled);
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
        .set_goal("Replacement".into(), vec![], vec![], "scope".into())
        .unwrap();
    engine
        .add_step(
            "verify".into(),
            "Replacement step".into(),
            StepType::Test,
            None,
            None,
        )
        .unwrap();
    let mut restored = RunEngine::new("project".into()).with_persistence(persistence);
    assert_eq!(
        restored
            .settle_admitted_check(admission.clone(), outcome(), None)
            .unwrap_err()
            .effect(),
        "unknown"
    );
    assert!(restored.steps[0].checks.is_empty());
    let check_result = restored
        .check_results
        .get(&admission.check_result_id)
        .unwrap()
        .unwrap();
    assert_eq!(check_result.stage, CheckResultStage::Applying);
    assert_eq!(check_result.outcome, Some(outcome()));
    assert!(check_result.settlement.is_none());
}

#[test]
fn finished_cycles_survive_reselection_and_reused_local_step_ids() {
    let (_directory, persistence, mut engine) = disk();
    engine
        .record_progress(
            None,
            ProgressType::Code,
            "original progress".into(),
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
    engine.complete_step("verify", vec![]).unwrap();
    engine.finalize(vec![], Some("finished".into())).unwrap();
    let original = engine.persisted_state();
    for scope in ["scope", "another"] {
        engine
            .set_goal("Next".into(), vec![], vec![], scope.into())
            .unwrap();
        engine
            .add_step(
                "verify".into(),
                "New step".into(),
                StepType::Test,
                None,
                None,
            )
            .unwrap();
        engine.start_step("verify").unwrap();
        engine.complete_step("verify", vec![]).unwrap();
        engine.finalize(vec![], None).unwrap();
    }
    let restored = RunEngine::new("project".into()).with_persistence(persistence);
    assert_eq!(restored.saved_cycles.len(), 2);
    let retained = &restored.saved_cycles[0];
    assert_eq!(retained.goal, original.goal.unwrap());
    assert_eq!(retained.steps, original.steps);
    assert_eq!(retained.execution.run_id, original.execution.run_id);
    assert_eq!(retained.execution.steps, original.execution.steps);
    assert_ne!(retained.execution.run_id, restored.execution.run_id);
    assert_ne!(
        retained.execution.steps["verify"],
        restored.execution.steps["verify"]
    );
    assert_eq!(
        restored
            .check_results
            .get(&admission.check_result_id)
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
        .set_goal("Other".into(), vec![], vec![], "other".into())
        .unwrap();
    engine.finalize(vec![], None).unwrap();
    let finished = engine.execution.run_id.clone();
    engine
        .set_goal("Ignored replacement".into(), vec![], vec![], "scope".into())
        .unwrap();
    assert_eq!(engine.execution.run_id, unfinished);
    assert_eq!(engine.steps[0].status, StepStatus::Active);
    assert_eq!(engine.saved_cycles.len(), 1);
    assert_eq!(engine.saved_cycles[0].execution.run_id, finished);
    engine
        .set_goal("New other".into(), vec![], vec![], "other".into())
        .unwrap();
    assert_ne!(engine.execution.run_id, finished);
    assert_eq!(engine.saved_cycles.len(), 2);
}

#[test]
fn finished_observations_keep_lanes_and_check_result_references_separate() {
    let (_directory, persistence, mut engine) = disk();
    let admission = admit(&mut engine);
    engine
        .settle_admitted_check(admission.clone(), outcome(), None)
        .unwrap();
    engine.complete_step("verify", vec![]).unwrap();
    engine.finalize(vec![], None).unwrap();
    let old = engine.observe().unwrap().value.current.unwrap();
    engine
        .set_goal("Next".into(), vec![], vec![], "next".into())
        .unwrap();
    let second_store = persistence.clone().for_execution("another");
    let mut other = RunEngine::new("project".into()).with_persistence(second_store);
    other
        .set_goal("Another lane".into(), vec![], vec![], "scope".into())
        .unwrap();
    let observed = engine.observe().unwrap();
    assert_eq!(observed.value.run(&old.run_id), Some(&old));
    assert_eq!(observed.value.retained.len(), 1);
    assert!(other.observe().unwrap().value.retained.is_empty());
    assert!(
        other
            .check_results
            .get(&admission.check_result_id)
            .unwrap()
            .is_none()
    );
    engine
        .compact_check_results(|| Ok(Default::default()))
        .unwrap();
    assert!(
        engine
            .check_results
            .get(&admission.check_result_id)
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
