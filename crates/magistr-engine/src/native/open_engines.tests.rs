use super::*;

#[test]
fn engine_open_uses_captured_validation_and_project_session_settings() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = NativeConfig::from_lookup(tmp.path().join("project"), |key| match key {
        "THINK_AND_SHIP_STRICT_MODE" | "THINK_AND_SHIP_ENABLE_SESSIONS" => Some("true".into()),
        _ => None,
    })
    .with_data_dir(tmp.path().join("store"))
    .persist(false);
    for project in ["first-project", "second-project"] {
        let (mut think, _, _, _) = open_engines(&cfg, project.into(), None);
        assert_eq!(think.status_snapshot(false)["default_session_id"], project);
        let step = serde_json::from_value(serde_json::json!({
            "step_number": 1, "estimated_total": 1, "purpose": "analysis",
            "context": "test", "thought": "Missing strict prefix",
            "rationale": "Missing strict prefix", "outcome": "test",
            "next_action": "Continue"
        }))
        .expect("step");
        assert!(
            think.process_step_native(step).is_err(),
            "captured strict validation must apply"
        );
    }
}
