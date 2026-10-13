use super::*;

#[tokio::test]
async fn an_unavailable_source_owner_is_refused_before_provider_startup() {
    let tmp = tempfile::tempdir().unwrap();
    let inputs = crate::config::inputs::ProjectInputs::from_lookup(tmp.path(), |_| None);
    let held = super::super::WorkspaceAdmission::admit(&inputs).await;
    assert!(held.hosted().is_some(), "{}", held.report());
    let config = AgentConfig::new("", tmp.path()).with_inputs(&inputs);
    let root = tmp.path().canonicalize().unwrap();
    let native = magistr_core::evidence::writer_config_from_env(&root);
    let address = SessionAddress {
        project: native.resolve_project_id(),
        workspace: root.display().to_string(),
        session: "one".into(),
    };
    let mut admissions = Admissions::default();
    let error = admissions
        .session(&config, &address)
        .await
        .err()
        .expect("source owner refused");
    assert!(
        error
            .to_string()
            .contains("working copy could not be opened"),
        "{error:#}"
    );
    admissions.close().await.unwrap();
    held.shutdown().await.unwrap();
}
