use super::*;

// A capture with no home: no user presets or plugins reach the scaffold.
fn inputs(root: &std::path::Path) -> ProjectInputs {
    ProjectInputs::from_lookup(root, |_| None)
}

#[tokio::test]
async fn force_reinit_keeps_mcp_project_config_and_state() {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();
    run_init(root, false, &inputs(root)).await.unwrap();
    let project_path = root.join(".magistr/project.json");
    let mut config: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&project_path).unwrap()).unwrap();
    config["future"] = json!({"keep": true});
    config["lifecycle_hooks"] = json!([]);
    let content = format!("{}\n", serde_json::to_string_pretty(&config).unwrap());
    std::fs::write(&project_path, &content).unwrap();
    let state = root.join(".magistr/state.json");
    std::fs::write(&state, "existing session").unwrap();

    let result = run_init(root, true, &inputs(root)).await.unwrap();
    assert_ne!(result.is_error, Some(true));
    assert_eq!(std::fs::read_to_string(&project_path).unwrap(), content);
    assert_eq!(std::fs::read_to_string(&state).unwrap(), "existing session");
}

#[tokio::test]
async fn force_reinit_reports_invalid_json_without_writes() {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();
    std::fs::write(root.join(".mcp.json"), "{broken").unwrap();
    let error = run_init(root, true, &inputs(root)).await.unwrap_err();
    assert!(error.to_string().contains(".mcp.json"), "{error}");
    assert_eq!(
        std::fs::read_to_string(root.join(".mcp.json")).unwrap(),
        "{broken"
    );
    assert!(!root.join(".magistr").exists());
    assert!(!root.join(".claude").exists());
}
