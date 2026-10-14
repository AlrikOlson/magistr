//! Executable baseline for the native workflow boundaries documented in
//! `docs/architecture/contracts.md`. These scenarios characterize the
//! admitted engine facade; they do not imply peer fencing.

use std::process::Command;

use magistr_engine::{
    NativeConfig, NativeEngines, NativeReader, ProjectAuthority, ShipGuardState, WriterRole,
};
use magistr_workflow::roadmap::domain::ChunkStatus;
use magistr_workflow::ship::domain::task::TaskType;

fn config(tmp: &tempfile::TempDir) -> NativeConfig {
    let root = tmp.path().join("project");
    std::fs::create_dir_all(&root).expect("project directory");
    NativeConfig::from_lookup(root, |_| None).with_data_dir(tmp.path().join("native-store"))
}

fn host(config: &NativeConfig, project_id: &str) -> NativeEngines {
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
fn one_execution_shares_selection_and_other_projects_keep_their_scopes() {
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
    assert!(refusal.to_string().contains("first-verify"), "{refusal}");
    assert_eq!(
        shared.ship_active_task_id().unwrap().unwrap(),
        "first-verify"
    );

    // Selecting another scope saves the first plan and selects its own tasks.
    assert_eq!(
        shared
            .ship_set_objective("Replacement".into(), vec![], vec![], "chunk:second".into())
            .expect("objective")
            .value,
        0
    );
    assert_eq!(
        owner.ship_objective().unwrap().unwrap().scope,
        "chunk:second"
    );
    assert!(owner.ship_active_task_id().unwrap().is_none());
    assert!(owner.ship_tasks().unwrap().is_empty());

    // Other clients reuse the admitted writer. A second writer role still
    // cannot acquire this project's authority while its owner is alive.
    assert_eq!(
        ProjectAuthority::admit_for_project(&config, "project-a".into(), WriterRole::Service)
            .expect_err("one project owner")
            .kind(),
        std::io::ErrorKind::WouldBlock
    );
    let reopened = owner.clone();
    assert_eq!(
        reopened.ship_objective().unwrap().unwrap().scope,
        "chunk:second"
    );
    assert!(reopened.ship_start_task("second-verify").is_err());
    objective_with_task(&reopened, "chunk:second", "second-scope-verify");
    assert_eq!(
        owner.ship_active_task_id().unwrap().unwrap(),
        "second-scope-verify"
    );
    owner
        .ship_set_objective("First plan".into(), vec![], vec![], "chunk:first".into())
        .expect("restore first scope");
    assert_eq!(
        reopened.ship_active_task_id().unwrap().unwrap(),
        "first-verify"
    );
    assert_eq!(reopened.ship_tasks().unwrap().len(), 2);

    let independent_root = tmp.path().join("project-b");
    std::fs::create_dir(&independent_root).expect("independent project");
    let independent_config = NativeConfig::from_lookup(independent_root, |_| None)
        .with_data_dir(tmp.path().join("native-store"));
    let independent = host(&independent_config, "project-b");
    assert!(independent.ship_objective().unwrap().is_none());
    objective_with_task(&independent, "chunk:independent", "independent-verify");
    assert_eq!(
        independent.ship_active_task_id().unwrap().unwrap(),
        "independent-verify"
    );
    assert_eq!(
        owner.ship_active_task_id().unwrap().unwrap(),
        "first-verify"
    );
}

/// A writer admitted on a configuration that carries no evidence capability
/// (what every test here opens) still composes the ship family as the
/// roadmap's completion authority: completion is refused with the missing
/// capability named, and neither the phase guard nor finalization changes
/// what that refusal decided.
#[test]
fn native_completion_refuses_without_evidence_while_phase_admission_and_finalization_keep_their_contracts()
 {
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
    let before = ShipGuardState::from_ship(
        engines.ship_objective().unwrap().as_ref(),
        &engines.ship_tasks().unwrap(),
    );
    assert!(before.phase_block("verify").is_none());
    assert!(before.phase_block("done").is_some());

    // No evidence capability, no completion — on the verb every adapter
    // lands on, in words that name what is missing.
    let refused = engines
        .roadmap_complete_chunk("fixture", None)
        .expect_err("no capability certifies the gates");
    assert!(
        refused
            .to_string()
            .contains("no verification evidence capability is composed"),
        "{refused}"
    );
    let refused = engines
        .roadmap_set_status("fixture", ChunkStatus::Done)
        .expect_err("done is never set by name");
    assert!(
        refused.to_string().contains("roadmap_complete_chunk"),
        "{refused}"
    );
    let chunk = engines.roadmap_chunk("fixture").unwrap().expect("chunk");
    assert_eq!(chunk.status, ChunkStatus::InProgress);
    assert!(chunk.cross_refs.is_empty());
    assert!(
        ShipGuardState::from_ship(
            engines.ship_objective().unwrap().as_ref(),
            &engines.ship_tasks().unwrap()
        )
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
    assert!(check.value.required && check.value.verified);
    assert!(!check.value.passed);
    assert_eq!(check.value.exit_code, Some(exit_code));
    let guard = ShipGuardState::from_ship(
        engines.ship_objective().unwrap().as_ref(),
        &engines.ship_tasks().unwrap(),
    );
    assert_eq!(guard.checks_recorded, 1);
    assert!(guard.phase_block("done").is_none());

    engines
        .ship_complete_task("fixture-verify", vec![])
        .expect("complete the task with its failed check retained");
    let report = engines
        .ship_finalize(vec![], Some("Characterization only".into()))
        .expect("finalize");
    assert_eq!(report.value.status, "shipped");
    assert!(
        report.value.warnings.iter().any(|warning| {
            warning.contains("required checks failed") && warning.contains("fixture-verify")
        }),
        "{report:?}"
    );

    let reopened = NativeReader::open_for_project(&config, "completion".into());
    assert_eq!(
        reopened.ship_objective().unwrap().unwrap().status,
        magistr_workflow::ship::domain::objective::ObjectiveStatus::Completed
    );
    assert!(!reopened.ship_tasks().unwrap()[0].checks[0].passed);
    let chunk = reopened
        .roadmap_chunk("fixture")
        .unwrap()
        .expect("persisted chunk");
    assert_eq!(chunk.status, ChunkStatus::InProgress);
    assert!(chunk.cross_refs.is_empty());
}
