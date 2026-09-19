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

/// The tool's check mode is the read-only surface for the startup repair:
/// a project whose recorded workflow runtime was deleted answers the
/// preserved runtime and the refused skills with their reasons, and the
/// check writes nothing.
#[tokio::test]
async fn check_mode_reports_the_startup_repair_plan_and_its_refusals_without_writing() {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();
    run_init(root, false, &inputs(root)).await.unwrap();
    let runtime = ".claude/skills/craft/references/runtime.md";
    std::fs::remove_file(root.join(runtime)).unwrap();
    let skill = root.join(".claude/skills/next/SKILL.md");
    let skill_before = std::fs::read_to_string(&skill).unwrap();
    let manifest_before = std::fs::read(root.join(".magistr/scaffold-manifest.json")).unwrap();

    let result = run_check(root).unwrap();
    assert_ne!(result.is_error, Some(true));
    let text = match &result.content[0] {
        magistr_mcp::ToolContent::Text { text } => text,
        _ => panic!("expected text content"),
    };
    let report: serde_json::Value = serde_json::from_str(text).unwrap();

    assert!(report["clean"].is_boolean());
    let repair = &report["repair"];
    assert_eq!(repair["operation"], "repair");
    assert!(repair["summary"]["refused"].as_u64().unwrap() >= 1);
    let entries = repair["files"].as_array().unwrap();
    let refused = entries
        .iter()
        .find(|e| e["path"] == ".claude/skills/next/SKILL.md")
        .unwrap();
    assert_eq!(refused["action"], "refused");
    assert_eq!(refused["intent"], "refuse");
    assert_eq!(
        refused["reason"],
        format!("depends on {runtime}, which is not available")
    );
    assert!(refused["diff"].is_null(), "the tool answer is a summary");
    let preserved = entries.iter().find(|e| e["path"] == runtime).unwrap();
    assert_eq!(preserved["action"], "preserved");
    assert_eq!(preserved["expected"], "absent");

    assert!(!root.join(runtime).exists(), "a check writes nothing");
    assert_eq!(std::fs::read_to_string(&skill).unwrap(), skill_before);
    assert_eq!(
        std::fs::read(root.join(".magistr/scaffold-manifest.json")).unwrap(),
        manifest_before
    );
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
