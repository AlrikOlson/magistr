use super::*;

#[test]
fn engine_open_uses_captured_validation_and_project_log_settings() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = NativeConfig::from_lookup(tmp.path().join("project"), |key| match key {
        "MAGISTR_WORKFLOW_STRICT_MODE" | "MAGISTR_WORKFLOW_ENABLE_LOGS" => Some("true".into()),
        _ => None,
    })
    .with_data_dir(tmp.path().join("store"))
    .persist(false);
    for project in ["first-project", "second-project"] {
        let (mut decision, _, _, _) = open_engines(&cfg, project.into(), None);
        assert_eq!(decision.status_snapshot(false)["default_log_id"], project);
        let note = serde_json::from_value(serde_json::json!({
            "note": 1, "estimated_total": 1, "purpose": "analysis",
            "context": "test", "thought": "Missing strict prefix",
            "rationale": "Missing strict prefix", "outcome": "test",
            "next_action": "Continue"
        }))
        .expect("note");
        assert!(
            decision.process_note_native(note).is_err(),
            "captured strict validation must apply"
        );
    }
}
