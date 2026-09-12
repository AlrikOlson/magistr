use super::*;
use magistr_engine::native::{
    CheckSettlement, ProjectAuthority, ReceiptSettlement, SourceFreshness, TaskType, WriterRole,
};

/// A host writer admitted the way production admits one: on a writer
/// config carrying the project's own evidence capability.
fn host(tmp: &tempfile::TempDir) -> (NativeEngines, PathBuf) {
    let root = tmp.path().join("project");
    std::fs::create_dir_all(root.join(".git")).expect("project directory");
    std::fs::write(root.join("notes.txt"), "alpha\n").expect("source");
    let store = tmp.path().join("store");
    let config = magistr_core::evidence::writer_config(&root, |_| None).with_data_dir(store);
    let engines =
        ProjectAuthority::admit_for_project(&config, "composition".into(), WriterRole::Host)
            .expect("host admitted")
            .open();
    (engines, root)
}

fn objective(engines: &NativeEngines, task: &str) {
    engines
        .ship_set_objective("Compose".into(), vec![], vec![], "chunk:test".into())
        .expect("objective");
    engines
        .ship_add_task(task.into(), "Verify".into(), TaskType::Test, None)
        .expect("task");
    engines.ship_start_task(task).expect("active task");
}

#[test]
fn admission_binds_the_task_and_the_source_before_the_run_and_settles_both() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (engines, root) = host(&tmp);
    objective(&engines, "verify");

    let admitted = AdmittedCheck::admit(&engines, &root, None, "test", "true".into(), true)
        .expect("admitted with source");
    let identities = admitted.identities();
    assert_eq!(identities.project_id, engines.project_id());
    assert_eq!(identities.task_id, "verify");
    assert!(!identities.task_instance.is_empty());
    assert!(!identities.run_id.is_empty());
    let workspace = identities.workspace.expect("source identity");
    assert_eq!(workspace.root, root.canonicalize().expect("canonical"));
    assert_eq!(
        workspace.generation, None,
        "no owner ever admitted this root"
    );
    assert_ne!(identities.run_id, workspace.root.to_string_lossy());
    let before = admitted.source().expect("observation").revision.clone();
    assert!(!admitted.receipt_id().is_empty());

    // An external edit while the "gate" runs: observed, never journaled.
    std::fs::write(root.join("notes.txt"), "beta\n").expect("external edit");

    let settled = admitted
        .settle(&engines, CheckExecution::Exited { code: 0 }, "ok".into())
        .expect("settled");
    assert!(settled.is_applied());
    let check = settled.check().expect("recorded check");
    assert!(check.passed && check.verified);
    assert_eq!(check.exit_code, Some(0));
    let receipt = settled.receipt();
    assert_eq!(receipt.source.as_ref().expect("source").revision, before);
    assert_eq!(receipt.settlement, Some(ReceiptSettlement::Applied));
    assert_eq!(
        settled.freshness(),
        Some(&SourceFreshness::Changed {
            paths: vec!["notes.txt (Modified)".into()]
        })
    );
    assert_eq!(engines.ship_active_task_id().as_deref(), Some("verify"));
}

#[test]
fn a_check_with_no_task_is_refused_before_any_bytes_are_hashed() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (engines, root) = host(&tmp);
    engines
        .ship_set_objective("Compose".into(), vec![], vec![], "chunk:test".into())
        .expect("objective");
    let refused = AdmittedCheck::admit(&engines, &root, None, "lint", "true".into(), true)
        .expect_err("no active task");
    assert!(
        matches!(&refused, CheckEvidenceError::Admission(reason) if reason.contains("no active task")),
        "{refused}"
    );
}

#[test]
fn a_superseded_objective_settles_stale_with_the_outcome_and_receipt_intact() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (engines, root) = host(&tmp);
    objective(&engines, "verify");
    let admitted = AdmittedCheck::admit(
        &engines,
        &root,
        Some("verify"),
        "check",
        "false".into(),
        true,
    )
    .expect("admitted");
    let original = admitted.target().clone();
    let receipt_id = admitted.receipt_id().to_owned();

    // The agent shipped and rolled the objective over while the gate ran;
    // the same task name in the new run is a different instance.
    engines.ship_finalize(vec![], None).expect("shipped");
    objective(&engines, "verify");

    let settled = admitted
        .settle(&engines, CheckExecution::Exited { code: 1 }, "red".into())
        .expect("settled, not an error");
    assert!(!settled.is_applied());
    match &settled.settlement {
        CheckSettlement::Stale {
            admission,
            outcome,
            receipt,
        } => {
            assert_eq!(admission.target, original);
            assert_eq!(outcome.exit_code, Some(1));
            assert!(!outcome.passed);
            assert!(outcome.verified);
            assert_eq!(receipt.id, receipt_id);
            assert_eq!(receipt.target, original);
            assert!(matches!(
                receipt.settlement,
                Some(ReceiptSettlement::Stale { .. })
            ));
        }
        CheckSettlement::Applied { .. } => panic!("a superseded task must not collect the result"),
    }
    assert_eq!(settled.freshness(), Some(&SourceFreshness::Unchanged));
    assert!(
        engines.ship_status()["checks"]
            .as_array()
            .is_some_and(Vec::is_empty),
        "nothing lands on the new instance"
    );
}

#[test]
fn a_timeout_and_a_cancel_are_recorded_as_what_they_were() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (engines, root) = host(&tmp);
    objective(&engines, "verify");
    for execution in [CheckExecution::TimedOut, CheckExecution::Cancelled] {
        let admitted = AdmittedCheck::admit(&engines, &root, None, "test", "true".into(), true)
            .expect("admitted");
        let settled = admitted
            .settle(&engines, execution.clone(), String::new())
            .expect("settled");
        let check = settled.check().expect("recorded");
        assert!(!check.passed && check.verified);
        assert_eq!(check.exit_code, None);
        assert_eq!(settled.receipt().execution, Some(execution));
    }
}

#[test]
fn an_unobservable_root_is_a_source_refusal_at_admission() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let absent = tmp.path().join("absent");
    let config = magistr_core::evidence::writer_config(&absent, |_| None)
        .with_data_dir(tmp.path().join("store"));
    let engines = ProjectAuthority::admit_for_project(&config, "absent".into(), WriterRole::Host)
        .expect("host admitted")
        .open();
    objective(&engines, "verify");
    let refused = AdmittedCheck::admit(&engines, &absent, None, "test", "true".into(), true)
        .expect_err("missing root");
    assert!(
        matches!(&refused, CheckEvidenceError::Source(reason) if reason.contains("does not exist")),
        "{refused}"
    );
    assert!(
        engines.ship_status()["checks"]
            .as_array()
            .is_some_and(Vec::is_empty),
        "nothing was recorded"
    );
}
