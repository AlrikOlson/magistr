use super::*;
use magistr_engine::native::{ProjectAuthority, StepType, VerificationMode, WriterRole};

#[test]
fn plans_read_complete_native_runs_and_preserve_absence_failure_and_identity() {
    let dir = tempfile::tempdir().unwrap();
    let inputs = ProjectInputs::from_lookup(dir.path(), |key| {
        (key == "MAGISTR_WORKFLOW_DATA_DIR").then(|| dir.path().join("data").display().to_string())
    });
    let native = inputs.workflow();
    let reader = NativeReader::open(native);
    assert!(read_runs(&reader, None, 20).unwrap().is_empty());
    let writer = ProjectAuthority::admit(native, WriterRole::Host)
        .unwrap()
        .open();
    writer
        .run_open_with_verification(
            "Completed work".into(),
            vec!["Authored requirement".into()],
            vec![],
            "first".into(),
            VerificationMode::Focused,
        )
        .unwrap();
    writer
        .add_step(
            "edit".into(),
            "A complete step".into(),
            StepType::Implement,
            None,
        )
        .unwrap();
    writer.start_step("edit").unwrap();
    writer.finish_step("edit", vec![]).unwrap();
    writer.run_finish(vec![], None).unwrap();
    let finished = read_runs(&reader, None, 20).unwrap().remove(0);
    writer
        .run_open_with_verification(
            "Current work".into(),
            vec![],
            vec![],
            "next".into(),
            VerificationMode::Focused,
        )
        .unwrap();
    writer
        .add_step(
            "edit".into(),
            "A new step".into(),
            StepType::Implement,
            None,
        )
        .unwrap();
    let all = read_runs(&reader, None, 20).unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].goal.description, "Current work");
    assert_eq!(all[1], finished);
    assert_ne!(all[0].step_instances["edit"], all[1].step_instances["edit"]);
    assert_eq!(
        read_runs(&reader, Some(cli::RunStatusFilter::Finished), 20).unwrap(),
        vec![finished]
    );
    assert_eq!(
        read_runs(&reader, Some(cli::RunStatusFilter::Active), 20)
            .unwrap()
            .len(),
        1
    );
    assert!(read_runs(&reader, None, 0).unwrap().is_empty());
    let path = native
        .data_dir()
        .join("run")
        .join("executions")
        .join(native.execution_context().key())
        .join("sessions")
        .join(format!("{}.json", native.resolve_project_id()));
    let bytes = std::fs::read(&path).unwrap();
    std::fs::write(&path, b"invalid plan").unwrap();
    assert!(read_runs(&reader, None, 20).is_err());
    std::fs::write(path, bytes).unwrap();
    assert_eq!(read_runs(&reader, None, 20).unwrap(), all);
}
