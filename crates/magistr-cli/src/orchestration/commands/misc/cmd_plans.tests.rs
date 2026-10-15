use super::*;
use magistr_engine::native::{ProjectAuthority, TaskType, VerificationMode, WriterRole};

#[test]
fn plans_read_complete_native_runs_and_preserve_absence_failure_and_identity() {
    let dir = tempfile::tempdir().unwrap();
    let inputs = ProjectInputs::from_lookup(dir.path(), |key| {
        (key == "THINK_AND_SHIP_DATA_DIR").then(|| dir.path().join("data").display().to_string())
    });
    let native = inputs.workflow();
    let reader = NativeReader::open(native);
    assert!(read_plans(&reader, None, 20).unwrap().is_empty());
    let writer = ProjectAuthority::admit(native, WriterRole::Host)
        .unwrap()
        .open();
    writer
        .ship_set_objective_with_verification(
            "Completed work".into(),
            vec!["Authored requirement".into()],
            vec![],
            "first".into(),
            VerificationMode::Focused,
        )
        .unwrap();
    writer
        .ship_add_task(
            "edit".into(),
            "A complete task".into(),
            TaskType::Implement,
            None,
        )
        .unwrap();
    writer.ship_start_task("edit").unwrap();
    writer.ship_complete_task("edit", vec![]).unwrap();
    writer.ship_finalize(vec![], None).unwrap();
    let finished = read_plans(&reader, None, 20).unwrap().remove(0);
    writer
        .ship_set_objective_with_verification(
            "Current work".into(),
            vec![],
            vec![],
            "next".into(),
            VerificationMode::Focused,
        )
        .unwrap();
    writer
        .ship_add_task(
            "edit".into(),
            "A new task".into(),
            TaskType::Implement,
            None,
        )
        .unwrap();
    let all = read_plans(&reader, None, 20).unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].objective.description, "Current work");
    assert_eq!(all[1], finished);
    assert_ne!(all[0].task_instances["edit"], all[1].task_instances["edit"]);
    assert_eq!(
        read_plans(&reader, Some(cli::PlanStatusFilter::Completed), 20).unwrap(),
        vec![finished]
    );
    assert_eq!(
        read_plans(&reader, Some(cli::PlanStatusFilter::Active), 20)
            .unwrap()
            .len(),
        1
    );
    assert!(read_plans(&reader, None, 0).unwrap().is_empty());
    let path = native
        .data_dir()
        .join("ship")
        .join("executions")
        .join(native.execution_context().key())
        .join("sessions")
        .join(format!("{}.json", native.resolve_project_id()));
    let bytes = std::fs::read(&path).unwrap();
    std::fs::write(&path, b"invalid plan").unwrap();
    assert!(read_plans(&reader, None, 20).is_err());
    std::fs::write(path, bytes).unwrap();
    assert_eq!(read_plans(&reader, None, 20).unwrap(), all);
}
