use super::*;
use magistr_engine::tas::{CheckSettlement, ProjectAuthority, TasConfig, TaskType, WriterRole};
use magistr_workspace::snapshot::{ChangeKind, SnapshotError, SnapshotValidation};

fn host(tmp: &tempfile::TempDir) -> NativeEngines {
    let root = tmp.path().join("project");
    std::fs::create_dir_all(&root).expect("project directory");
    let config = TasConfig::from_lookup(root, |_| None).with_data_dir(tmp.path().join("store"));
    ProjectAuthority::admit_for_project(&config, "composition".into(), WriterRole::Host)
        .expect("host admitted")
        .open()
}

fn workspace(tmp: &tempfile::TempDir) -> PathBuf {
    let root = tmp.path().join("workspace");
    std::fs::create_dir_all(root.join(".git")).expect("repository marker");
    std::fs::write(root.join("notes.txt"), "alpha\n").expect("source");
    root
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
    let engines = host(&tmp);
    let root = workspace(&tmp);
    objective(&engines, "verify");

    let admitted = AdmittedCheck::admit(
        &engines,
        &root,
        None,
        "test",
        "true".into(),
        true,
        Some(SelectionPolicy::default()),
    )
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
    let before = admitted.source().expect("snapshot").revision();

    // An external edit while the "gate" runs: observed, never journaled.
    std::fs::write(root.join("notes.txt"), "beta\n").expect("external edit");

    let settled = admitted.settle(&engines, 0, "ok".into()).expect("settled");
    assert!(settled.is_applied());
    let check = settled.check().expect("recorded check");
    assert!(check.passed && check.verified);
    assert_eq!(check.exit_code, Some(0));
    let source = settled.source.expect("source evidence");
    assert_eq!(source.revision(), before);
    match &source.validation {
        SnapshotValidation::Changed(changes) => {
            assert_eq!(changes.len(), 1);
            assert_eq!(changes[0].path, "notes.txt");
            assert_eq!(changes[0].kind, ChangeKind::Modified);
        }
        SnapshotValidation::Unchanged => panic!("the external edit must be observed"),
    }
    assert!(!source.is_unchanged());
    assert_eq!(engines.ship_active_task_id().as_deref(), Some("verify"));
}

#[test]
fn a_check_with_no_task_is_refused_before_any_bytes_are_hashed() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let engines = host(&tmp);
    let root = workspace(&tmp);
    engines
        .ship_set_objective("Compose".into(), vec![], vec![], "chunk:test".into())
        .expect("objective");
    let refused = AdmittedCheck::admit(
        &engines,
        &root,
        None,
        "lint",
        "true".into(),
        true,
        Some(SelectionPolicy::default()),
    )
    .expect_err("no active task");
    assert!(
        matches!(&refused, CheckEvidenceError::Admission(reason) if reason.contains("no active task")),
        "{refused}"
    );
}

#[test]
fn a_superseded_objective_settles_stale_with_the_outcome_and_source_intact() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let engines = host(&tmp);
    let root = workspace(&tmp);
    objective(&engines, "verify");
    let admitted = AdmittedCheck::admit(
        &engines,
        &root,
        Some("verify"),
        "check",
        "false".into(),
        true,
        Some(SelectionPolicy::default()),
    )
    .expect("admitted");
    let original = admitted.target().clone();

    // The agent shipped and rolled the objective over while the gate ran;
    // the same task name in the new run is a different instance.
    engines.ship_finalize(vec![], None).expect("shipped");
    objective(&engines, "verify");

    let settled = admitted
        .settle(&engines, 1, "red".into())
        .expect("settled, not an error");
    assert!(!settled.is_applied());
    match &settled.settlement {
        CheckSettlement::Stale { admission, outcome } => {
            assert_eq!(admission.target, original);
            assert_eq!(outcome.exit_code, Some(1));
            assert!(!outcome.passed);
            assert!(outcome.verified);
        }
        CheckSettlement::Applied { .. } => panic!("a superseded task must not collect the result"),
    }
    assert!(settled.source.expect("source").is_unchanged());
    assert!(
        engines.ship_status()["checks"]
            .as_array()
            .is_some_and(Vec::is_empty),
        "nothing lands on the new instance"
    );
}

#[test]
fn without_a_policy_no_source_is_observed() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let engines = host(&tmp);
    let root = workspace(&tmp);
    objective(&engines, "verify");
    let admitted = AdmittedCheck::admit(&engines, &root, None, "lint", "true".into(), false, None)
        .expect("admitted");
    assert!(admitted.source().is_none());
    assert!(admitted.identities().workspace.is_none());
    let settled = admitted
        .settle(&engines, 0, String::new())
        .expect("settled");
    assert!(settled.is_applied());
    assert!(settled.source.is_none());
    assert!(!settled.check().expect("check").required);
}

#[test]
fn an_unobservable_root_is_a_source_refusal_after_admission() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let engines = host(&tmp);
    objective(&engines, "verify");
    let refused = AdmittedCheck::admit(
        &engines,
        &tmp.path().join("absent"),
        None,
        "test",
        "true".into(),
        true,
        Some(SelectionPolicy::default()),
    )
    .expect_err("missing root");
    assert!(
        matches!(
            refused,
            CheckEvidenceError::Source(SnapshotError::RootMissing(_))
        ),
        "{refused}"
    );
}
