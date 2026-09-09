use super::*;

#[tokio::test]
async fn shared_connection_refuses_an_unsupported_session_provider_before_spawn() {
    let cwd = tempfile::tempdir().unwrap();
    let config = AgentConfig::new("must never reach an agent", cwd.path())
        .provider("unknown-runtime")
        .resume("do-not-load-on-another-runtime");
    let error = match connect_transport(&config).await {
        Ok(_) => panic!("an unavailable provider must not create a connection"),
        Err(error) => error.to_string(),
    };
    assert!(
        error.contains("Unsupported agent provider \"unknown-runtime\""),
        "{error}"
    );
    assert_eq!(std::fs::read_dir(cwd.path()).unwrap().count(), 0);
}
