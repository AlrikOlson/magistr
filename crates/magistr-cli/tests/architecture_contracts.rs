//! Executable baseline for the native workflow boundaries documented in
//! `docs/architecture/contracts.md`. These scenarios characterize the
//! admitted engine facade; they do not imply peer fencing.

use std::process::Command;

use magistr_engine::{
    NativeConfig, NativeEngines, NativeReader, ProjectAuthority, RunGuardState, WriterRole,
};
use magistr_workflow::roadmap::domain::ChunkStatus;
use magistr_workflow::run::domain::step::StepType;

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

fn goal_with_step(engines: &NativeEngines, scope: &str, step: &str) {
    engines
        .run_open(
            "Characterize native state".into(),
            vec![],
            vec![],
            scope.into(),
        )
        .expect("goal");
    engines
        .add_step(
            step.into(),
            "Verify the contract".into(),
            StepType::Test,
            None,
        )
        .expect("add step");
    engines.start_step(step).expect("start the step");
}

#[test]
fn one_execution_shares_selection_and_other_projects_keep_their_scopes() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let config = config(&tmp);
    let owner = host(&config, "project-a");
    let shared = owner.clone();
    goal_with_step(&owner, "chunk:first", "first-verify");

    shared
        .add_step(
            "second-verify".into(),
            "Second step".into(),
            StepType::Test,
            None,
        )
        .expect("add step");
    let refusal = shared
        .start_step("second-verify")
        .expect_err("another step is already active in this project");
    assert!(refusal.to_string().contains("first-verify"), "{refusal}");
    assert_eq!(
        shared.run_active_step_id().unwrap().unwrap(),
        "first-verify"
    );

    // Selecting another scope saves the first plan and selects its own steps.
    assert_eq!(
        shared
            .run_open("Replacement".into(), vec![], vec![], "chunk:second".into())
            .expect("goal")
            .value,
        0
    );
    assert_eq!(owner.run_goal().unwrap().unwrap().scope, "chunk:second");
    assert!(owner.run_active_step_id().unwrap().is_none());
    assert!(owner.run_steps().unwrap().is_empty());

    // Other clients reuse the admitted writer. A second writer role still
    // cannot acquire this project's authority while its owner is alive.
    assert_eq!(
        ProjectAuthority::admit_for_project(&config, "project-a".into(), WriterRole::Service)
            .expect_err("one project owner")
            .kind(),
        std::io::ErrorKind::WouldBlock
    );
    let reopened = owner.clone();
    assert_eq!(reopened.run_goal().unwrap().unwrap().scope, "chunk:second");
    assert!(reopened.start_step("second-verify").is_err());
    goal_with_step(&reopened, "chunk:second", "second-scope-verify");
    assert_eq!(
        owner.run_active_step_id().unwrap().unwrap(),
        "second-scope-verify"
    );
    owner
        .run_open("First plan".into(), vec![], vec![], "chunk:first".into())
        .expect("restore first scope");
    assert_eq!(
        reopened.run_active_step_id().unwrap().unwrap(),
        "first-verify"
    );
    assert_eq!(reopened.run_steps().unwrap().len(), 2);

    let independent_root = tmp.path().join("project-b");
    std::fs::create_dir(&independent_root).expect("independent project");
    let independent_config = NativeConfig::from_lookup(independent_root, |_| None)
        .with_data_dir(tmp.path().join("native-store"));
    let independent = host(&independent_config, "project-b");
    assert!(independent.run_goal().unwrap().is_none());
    goal_with_step(&independent, "chunk:independent", "independent-verify");
    assert_eq!(
        independent.run_active_step_id().unwrap().unwrap(),
        "independent-verify"
    );
    assert_eq!(owner.run_active_step_id().unwrap().unwrap(), "first-verify");
}

/// A writer admitted on a configuration that carries no evidence capability
/// (what every test here opens) still composes the run family as the
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
    goal_with_step(&engines, "chunk:fixture", "fixture-verify");
    let before = RunGuardState::from_run(
        engines.run_goal().unwrap().as_ref(),
        &engines.run_steps().unwrap(),
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
        RunGuardState::from_run(
            engines.run_goal().unwrap().as_ref(),
            &engines.run_steps().unwrap()
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
        .run_record_gate(
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
    let guard = RunGuardState::from_run(
        engines.run_goal().unwrap().as_ref(),
        &engines.run_steps().unwrap(),
    );
    assert_eq!(guard.checks_recorded, 1);
    assert!(guard.phase_block("done").is_none());

    engines
        .finish_step("fixture-verify", vec![])
        .expect("complete the step with its failed check retained");
    let report = engines
        .run_finish(vec![], Some("Characterization only".into()))
        .expect("finalize");
    assert_eq!(report.value.status, "finished");
    assert!(
        report.value.warnings.iter().any(|warning| {
            warning.contains("required checks failed") && warning.contains("fixture-verify")
        }),
        "{report:?}"
    );

    let reopened = NativeReader::open_for_project(&config, "completion".into());
    assert_eq!(
        reopened.run_goal().unwrap().unwrap().status,
        magistr_workflow::run::domain::goal::GoalStatus::Finished
    );
    assert!(!reopened.run_steps().unwrap()[0].checks[0].passed);
    let chunk = reopened
        .roadmap_chunk("fixture")
        .unwrap()
        .expect("persisted chunk");
    assert_eq!(chunk.status, ChunkStatus::InProgress);
    assert!(chunk.cross_refs.is_empty());
}
