use super::*;
use crate::scaffold::{FileAction, integrations, manifest::ScaffoldManifest};

fn fixture(agents: &[&str]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let selected = agents.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    ScaffoldPipeline::headless(crate::types::Stack::default())
        .with_agents(&selected)
        .generate(dir.path())
        .unwrap();
    dir
}

#[test]
fn explicit_repair_restores_missing_selected_files_and_preserves_customization() {
    let tmp = fixture(&["claude"]);
    let root = tmp.path();
    let customized = ".claude/rules/tools.md";
    let missing = ".claude/skills/craft/references/runtime.md";
    std::fs::write(root.join(customized), "My local tool guide\n").unwrap();
    std::fs::remove_file(root.join(missing)).unwrap();
    let prepared = prepare(root, &ProjectInputs::from_lookup(root, |_| None)).unwrap();
    assert_eq!(
        prepared.changes().get(customized).unwrap().action,
        FileAction::Preserved
    );
    assert_eq!(
        prepared.changes().get(missing).unwrap().action,
        FileAction::Create
    );
    assert!(!root.join(missing).exists(), "preview is read-only");
    prepared.apply().unwrap();
    assert!(root.join(missing).is_file());
    assert_eq!(
        std::fs::read_to_string(root.join(customized)).unwrap(),
        "My local tool guide\n"
    );
    assert!(!root.join(".codex").exists());
    assert!(!root.join(".cursor").exists());
    assert!(repair(root).unwrap().is_empty());
}

#[test]
fn reviewed_setup_refuses_a_file_changed_after_preview() {
    let tmp = fixture(&["claude"]);
    let root = tmp.path();
    let path = ".claude/rules/tools.md";
    let mut manifest = ScaffoldManifest::load(root);
    std::fs::write(root.join(path), "Recorded older guide\n").unwrap();
    manifest.record(path, "Recorded older guide\n");
    manifest.save(root).unwrap();
    let prepared = prepare(root, &ProjectInputs::from_lookup(root, |_| None)).unwrap();
    assert_eq!(
        prepared.changes().get(path).unwrap().action,
        FileAction::Overwrite
    );
    std::fs::write(root.join(path), "Changed after review\n").unwrap();
    assert!(prepared.apply().is_err());
    assert_eq!(
        std::fs::read_to_string(root.join(path)).unwrap(),
        "Changed after review\n"
    );
}

#[test]
fn repair_requires_valid_project_and_shared_configuration() {
    let tmp = tempfile::tempdir().unwrap();
    assert!(repair(tmp.path()).is_err());
    assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 0);
    let tmp = fixture(&["claude"]);
    let root = tmp.path();
    let before = std::fs::read(root.join(".claude/rules/tools.md")).unwrap();
    std::fs::write(root.join(".mcp.json"), "{broken").unwrap();
    assert!(repair(root).is_err());
    assert_eq!(
        std::fs::read(root.join(".claude/rules/tools.md")).unwrap(),
        before
    );
    assert_eq!(
        std::fs::read_to_string(root.join(".mcp.json")).unwrap(),
        "{broken"
    );
}

#[test]
fn removal_preserves_settings_work_user_files_and_unrelated_connections() {
    let tmp = fixture(&["codex", "claude"]);
    let root = tmp.path();
    let config = std::fs::read(root.join(".magistr/project.json")).unwrap();
    let identity = std::fs::read(root.join(".magistr/project.json")).unwrap();
    std::fs::write(root.join(".claude/rules/tools.md"), "Custom instructions\n").unwrap();
    std::fs::write(root.join("source.txt"), "user source").unwrap();
    let mut connections: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join(".mcp.json")).unwrap()).unwrap();
    connections["mcpServers"]["external"] = serde_json::json!({"command":"external","args":[]});
    std::fs::write(root.join(".mcp.json"), connections.to_string()).unwrap();
    let plan = integrations::removal_plan(root).unwrap();
    integrations::apply_removal(root, plan).unwrap();
    assert_eq!(
        std::fs::read(root.join(".magistr/project.json")).unwrap(),
        config
    );
    assert_eq!(
        std::fs::read(root.join(".magistr/project.json")).unwrap(),
        identity
    );
    assert_eq!(
        std::fs::read_to_string(root.join(".claude/rules/tools.md")).unwrap(),
        "Custom instructions\n"
    );
    assert!(root.join("source.txt").exists());
    let connections: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join(".mcp.json")).unwrap()).unwrap();
    assert!(connections["mcpServers"].get("magistr").is_none());
    assert_eq!(connections["mcpServers"]["external"]["command"], "external");
    let plan = integrations::removal_plan(root).unwrap();
    assert!(integrations::apply_removal(root, plan).unwrap().is_empty());
}

#[test]
fn declined_hooks_are_absent_in_both_the_reviewed_plan_and_configuration() {
    let tmp = tempfile::tempdir().unwrap();
    let pipeline = ScaffoldPipeline::with_selections(
        crate::types::Stack::default(),
        crate::scaffold::ScaffoldSelections {
            lifecycle_hooks: Some(Vec::new()),
            selected_agents: vec!["portable".into()],
            ..Default::default()
        },
    );
    let prepared = pipeline.prepare(tmp.path()).unwrap();
    let expected = prepared
        .changes()
        .get(".magistr/project.json")
        .unwrap()
        .new_content
        .clone();
    prepared.apply().unwrap();
    assert_eq!(
        std::fs::read_to_string(tmp.path().join(".magistr/project.json")).unwrap(),
        expected
    );
    let config = crate::validate::load_project_config(tmp.path()).unwrap();
    assert!(config.lifecycle_hooks.is_empty());
    assert!(!tmp.path().join(".claude").exists());
    assert!(!tmp.path().join(".codex").exists());
}
