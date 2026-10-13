use super::*;

#[tokio::test]
async fn empty_registry_configures_each_path_only_when_opened() {
    let tmp = tempfile::tempdir().unwrap();
    let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
    let observed = Arc::clone(&calls);
    let registry = SessionRegistry::start(move |root| {
        observed.lock().unwrap().push(root.to_path_buf());
        Err(eyre::eyre!("Provider unavailable"))
    });
    let client = registry.client();
    assert!(client.list().await.unwrap().is_empty());
    assert!(calls.lock().unwrap().is_empty());
    assert!(client.open(tmp.path()).await.is_err());
    assert!(
        calls.lock().unwrap().is_empty(),
        "unprepared paths never configure an agent"
    );
    std::fs::create_dir(tmp.path().join(".magistr")).unwrap();
    std::fs::write(tmp.path().join(".magistr/project.json"), "invalid").unwrap();
    assert!(client.open(tmp.path()).await.is_err());
    assert!(
        calls.lock().unwrap().is_empty(),
        "invalid settings never configure an agent"
    );
    std::fs::write(
        tmp.path().join(".magistr/project.json"),
        serde_json::json!({
 "project": {"name": "fixture", "type": "cli"},
 "stack": {"language": "unknown", "framework": "none", "package_manager": "unknown", "test_framework": "none", "linter": "none", "formatter": "none"},
 "quality_gate": {"check": "", "test": "", "lint": ""},
 "format": {"command": "", "extensions": []},
 "testing": {"companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside"},
 "reference_repos": [], "state_prefix": "fixture", "roadmap_exports": false
}).to_string(),
    )
    .unwrap();
    for _ in 0..2 {
        assert!(
            client
                .open(tmp.path())
                .await
                .unwrap_err()
                .contains("Provider unavailable")
        );
        assert!(client.list().await.unwrap().is_empty());
    }
    assert_eq!(
        *calls.lock().unwrap(),
        vec![tmp.path().canonicalize().unwrap(); 2]
    );
    registry.shutdown().await.unwrap();
}

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
