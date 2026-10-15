use super::*;
use magistr_engine::native::{NativeReader, ProjectAuthority, VerificationMode, WriterRole};

#[tokio::test]
async fn absent_and_finished_work_do_not_launch_and_unreadable_work_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let inputs = crate::config::inputs::ProjectInputs::from_lookup(dir.path(), |key| {
        (key == "THINK_AND_SHIP_DATA_DIR").then(|| dir.path().join("data").display().to_string())
    });
    let native = inputs.workflow();
    let run = || {
        cmd_next_continue(
            dir.path(),
            agent::Backend::Acp,
            crate::permissions::PermissionProfile::Cautious,
            &inputs,
        )
    };
    // No project config or provider is needed when there is nothing to resume.
    run().await.unwrap();
    let writer = ProjectAuthority::admit(native, WriterRole::Host)
        .unwrap()
        .open();
    writer
        .ship_set_objective_with_verification(
            "Finished".into(),
            vec![],
            vec![],
            "finished".into(),
            VerificationMode::Focused,
        )
        .unwrap();
    writer.ship_finalize(vec![], None).unwrap();
    let reader = NativeReader::open(native);
    let finished = reader.ship_snapshot().unwrap();
    run().await.unwrap();
    assert_eq!(reader.ship_snapshot().unwrap(), finished);
    let path = native
        .data_dir()
        .join("ship")
        .join("executions")
        .join(native.execution_context().key())
        .join("sessions")
        .join(format!("{}.json", native.resolve_project_id()));
    let bytes = std::fs::read(&path).unwrap();
    std::fs::write(&path, b"unreadable").unwrap();
    assert!(run().await.is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"unreadable");
    std::fs::write(path, bytes).unwrap();
    run().await.unwrap();
}
