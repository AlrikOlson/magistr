//! Executable baseline for the native workflow boundaries documented in
//! `docs/architecture/contracts.md`. These scenarios characterize the
//! admitted engine facade; they do not imply peer fencing.

use std::process::Command;

use magistr_engine::{
    NativeEngines, NativeReader, ProjectAuthority, ShipGuardState, TasConfig, WriterRole,
};
use think_and_ship::roadmap::domain::ChunkStatus;
use think_and_ship::ship::domain::task::TaskType;

fn config(tmp: &tempfile::TempDir) -> TasConfig {
    let root = tmp.path().join("project");
    std::fs::create_dir_all(&root).expect("project directory");
    TasConfig::from_lookup(root, |_| None).with_data_dir(tmp.path().join("native-store"))
}

fn host(config: &TasConfig, project_id: &str) -> NativeEngines {
    ProjectAuthority::admit_for_project(config, project_id.into(), WriterRole::Host)
        .expect("host admitted")
        .open()
}

fn objective_with_task(engines: &NativeEngines, scope: &str, task: &str) {
    engines
        .ship_set_objective(
            "Characterize native state".into(),
            vec![],
            vec![],
            scope.into(),
        )
        .expect("objective");
    engines
        .ship_add_task(
            task.into(),
            "Verify the contract".into(),
            TaskType::Test,
            None,
        )
        .expect("add task");
    engines.ship_start_task(task).expect("start the task");
}

#[test]
fn a_project_shares_one_objective_and_active_task_while_another_project_is_independent() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let config = config(&tmp);
    let owner = host(&config, "project-a");
    let shared = owner.clone();
    objective_with_task(&owner, "chunk:first", "first-verify");

    shared
        .ship_add_task(
            "second-verify".into(),
            "Second task".into(),
            TaskType::Test,
            None,
        )
        .expect("add task");
    let refusal = shared
        .ship_start_task("second-verify")
        .expect_err("another task is already active in this project");
    assert!(refusal.contains("first-verify"), "{refusal}");
    assert_eq!(shared.ship_status()["active_task"]["id"], "first-verify");

    // A scope describes the one project objective; it does not isolate a
    // second run. Replacing an unfinished objective keeps its existing tasks.
    assert_eq!(
        shared
            .ship_set_objective("Replacement".into(), vec![], vec![], "chunk:second".into())
            .expect("objective"),
        0
    );
    assert_eq!(owner.ship_status()["objective"]["scope"], "chunk:second");
    assert_eq!(owner.ship_status()["active_task"]["id"], "first-verify");
    assert_eq!(owner.ship_tasks().len(), 2);

    // Reopening the same project in the other admitted role reads the
    // persisted shared objective and task. This sequential observation is
    // not a concurrent-writer guarantee.
    let reopened =
        ProjectAuthority::admit_for_project(&config, "project-a".into(), WriterRole::Service)
            .expect("service admitted")
            .open();
    assert_eq!(reopened.ship_status()["objective"]["scope"], "chunk:second");
    assert!(reopened.ship_start_task("second-verify").is_err());

    let independent = host(&config, "project-b");
    assert!(independent.ship_status()["objective"].is_null());
    objective_with_task(&independent, "chunk:independent", "independent-verify");
    assert_eq!(
        independent.ship_status()["active_task"]["id"],
        "independent-verify"
    );
    assert_eq!(owner.ship_status()["active_task"]["id"], "first-verify");
}

#[test]
fn native_completion_phase_admission_and_finalization_have_distinct_evidence_contracts() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let config = config(&tmp);
    let engines = host(&config, "completion");
    engines
        .roadmap_add_chunk(
            "fixture".into(),
            "Characterize completion".into(),
            String::new(),
            ChunkStatus::Pending,
            100,
            "Native completion and verification have separate boundaries.".into(),
            vec!["A real required check passes.".into()],
            vec![],
            None,
            None,
        )
        .expect("add chunk");
    engines.roadmap_start_chunk("fixture").expect("start chunk");
    objective_with_task(&engines, "chunk:fixture", "fixture-verify");
    let before = ShipGuardState::from_status(&engines.ship_status());
    assert!(before.phase_block("verify").is_none());
    assert!(before.phase_block("done").is_some());

    // The native lifecycle accepts completion without consulting ship checks
    // or requiring the optional proof-of-ship reference.
    let done = engines
        .roadmap_complete_chunk("fixture", None)
        .expect("native lifecycle completion");
    assert_eq!(done.status, ChunkStatus::Done);
    assert!(done.cross_refs.is_empty());
    assert!(
        ShipGuardState::from_status(&engines.ship_status())
            .phase_block("done")
            .is_some()
    );

    // Record an actual failing process result through the host-facing facade.
    // An invalid CLI option exits before opening any project workflow state.
    let binary = env!("CARGO_BIN_EXE_magistr");
    let argument = "--architecture-contract-invalid-option";
    let output = Command::new(binary)
        .arg(argument)
        .current_dir(config.project_root())
        .output()
        .expect("run the failing fixture command");
    assert!(!output.status.success(), "the fixture command must fail");
    let exit_code = output.status.code().expect("normal process exit");
    let check = engines
        .ship_record_gate(
            None,
            "test",
            format!("{binary} {argument}"),
            exit_code,
            String::from_utf8_lossy(&output.stderr).into_owned(),
            true,
            None,
        )
        .expect("record the failed command");
    assert!(check.required && check.verified);
    assert!(!check.passed);
    assert_eq!(check.exit_code, Some(exit_code));
    let guard = ShipGuardState::from_status(&engines.ship_status());
    assert_eq!(guard.checks_recorded, 1);
    assert!(guard.phase_block("done").is_none());

    engines
        .ship_complete_task("fixture-verify", vec![])
        .expect("complete the task with its failed check retained");
    let report = engines
        .ship_finalize(vec![], Some("Characterization only".into()))
        .expect("finalize");
    assert_eq!(report["status"], "shipped");
    let warnings = report["warnings"]
        .as_array()
        .expect("finalization warnings");
    assert!(
        warnings.iter().any(|warning| {
            warning.as_str().is_some_and(|warning| {
                warning.contains("required checks failed") && warning.contains("fixture-verify")
            })
        }),
        "{report}"
    );

    let reopened = NativeReader::open_for_project(&config, "completion".into());
    assert_eq!(reopened.ship_status()["objective"]["status"], "completed");
    assert_eq!(reopened.ship_status()["checks"][0]["passed"], false);
    let chunk = reopened.roadmap_chunk("fixture").expect("persisted chunk");
    assert_eq!(chunk.status, ChunkStatus::Done);
    assert!(chunk.cross_refs.is_empty());
}
