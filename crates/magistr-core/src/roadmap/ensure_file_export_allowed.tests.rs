use super::*;

#[test]
fn roadmap_exports_default_to_allowed_and_observe_live_policy_changes() {
    let root = tempfile::tempdir().expect("project");
    assert!(ensure_file_export_allowed(root.path()).is_ok());
    let path = crate::paths::project_json(root.path());
    std::fs::create_dir_all(path.parent().expect("config parent")).expect("config directory");
    for allowed in [None, Some(false), Some(true), Some(false)] {
        let value = allowed.map_or_else(
            || serde_json::json!({}),
            |enabled| serde_json::json!({"roadmap_exports": enabled}),
        );
        std::fs::write(&path, value.to_string()).expect("policy");
        assert_eq!(
            ensure_file_export_allowed(root.path()).is_ok(),
            allowed.unwrap_or(true)
        );
    }
}

#[test]
fn roadmap_exports_refuse_invalid_configuration() {
    let root = tempfile::tempdir().expect("project");
    let path = crate::paths::project_json(root.path());
    std::fs::create_dir_all(path.parent().expect("config parent")).expect("config directory");
    for raw in ["{", r#"{"roadmap_exports":"false"}"#] {
        std::fs::write(&path, raw).expect("policy");
        assert!(ensure_file_export_allowed(root.path()).is_err());
    }
}
