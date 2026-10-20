use super::*;
use crate::scaffold::pipeline::ScaffoldPipeline;

fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    ScaffoldPipeline::headless(crate::types::Stack::default())
        .with_agents(&["codex".into(), "claude".into()])
        .generate(dir.path())
        .unwrap();
    dir
}

fn old_settings(root: &Path) -> String {
    let path = root.join(".magistr/project.json");
    let mut doc: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    doc["roadmap_backend"] = "think-and-ship".into();
    doc["quality_gate"]["check"] = "my-custom-check".into();
    doc["my_extension"] = serde_json::json!({"keep": [1,2,3]});
    let before = format!("{}\n", serde_json::to_string_pretty(&doc).unwrap());
    std::fs::write(path, &before).unwrap();
    before
}

#[test]
fn repair_previews_known_upgrades_then_preserves_custom_values_and_originals() {
    let dir = fixture();
    let root = dir.path();
    let before = old_settings(root);
    let mcp = serde_json::json!({"mcpServers":{
        "magistr":{"command":"magistr","args":["mcp-launch","magistr","."],"type":"stdio","env":{"MY_SETTING":"keep"}},
        "unrelated":{"command":"custom","args":["--keep"]}
    }});
    std::fs::write(root.join(".mcp.json"), mcp.to_string()).unwrap();
    let codex = "# Keep this comment\n[mcp_servers.magistr]\ncommand = 'magistr'\nargs = ['mcp-launch', 'magistr', '.']\nstartup_timeout_sec = 123\n[mcp_servers.unrelated]\ncommand = 'custom'\n";
    std::fs::write(root.join(".codex/config.toml"), codex).unwrap();
    let prepared = crate::scaffold::repair::prepare(
        root,
        &crate::config::inputs::ProjectInputs::from_lookup(root, |_| None),
    )
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(root.join(".magistr/project.json")).unwrap(),
        before
    );
    assert!(!root.join(".magistr/upgrades").exists());
    assert_eq!(
        prepared
            .changes()
            .get(".magistr/project.json")
            .unwrap()
            .action,
        FileAction::Overwrite
    );
    prepared.apply().unwrap();
    let doc: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join(".magistr/project.json")).unwrap())
            .unwrap();
    assert_eq!(doc["roadmap_backend"], "native");
    assert_eq!(doc["quality_gate"]["check"], "my-custom-check");
    assert_eq!(doc["my_extension"], serde_json::json!({"keep":[1,2,3]}));
    let after: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join(".mcp.json")).unwrap()).unwrap();
    assert_eq!(
        after["mcpServers"]["magistr"]["args"],
        serde_json::json!(["mcp", "--dir", "."])
    );
    assert_eq!(
        after["mcpServers"]["magistr"]["env"],
        mcp["mcpServers"]["magistr"]["env"]
    );
    assert_eq!(
        after["mcpServers"]["unrelated"],
        mcp["mcpServers"]["unrelated"]
    );
    let after = std::fs::read_to_string(root.join(".codex/config.toml")).unwrap();
    assert!(after.contains("# Keep this comment"));
    let parsed: toml::Value = toml::from_str(&after).unwrap();
    assert_eq!(
        parsed["mcp_servers"]["magistr"]["startup_timeout_sec"].as_integer(),
        Some(123)
    );
    assert_eq!(
        parsed["mcp_servers"]["magistr"]["args"][0].as_str(),
        Some("mcp")
    );
    let hash = crate::scaffold::manifest::content_hash(&before);
    assert_eq!(
        std::fs::read_to_string(root.join(format!(
            ".magistr/upgrades/configuration/{hash}/.magistr/project.json"
        )))
        .unwrap(),
        before
    );
    assert!(ProjectUpgrade::plan(root).unwrap().files.is_empty());
    assert!(crate::scaffold::repair::repair(root).unwrap().is_empty());
}

#[test]
fn automatic_apply_is_idempotent_and_does_not_generate_other_setup_files() {
    let dir = fixture();
    let before = old_settings(dir.path());
    std::fs::remove_file(dir.path().join(".claude/rules/tools.md")).unwrap();
    ProjectUpgrade::apply(dir.path()).unwrap();
    let after = std::fs::read(dir.path().join(".magistr/project.json")).unwrap();
    ProjectUpgrade::apply(dir.path()).unwrap();
    assert_eq!(
        std::fs::read(dir.path().join(".magistr/project.json")).unwrap(),
        after
    );
    assert!(!dir.path().join(".claude/rules/tools.md").exists());
    assert_ne!(after, before.as_bytes());
}

#[test]
fn unknown_config_and_backup_conflicts_leave_originals_untouched() {
    let dir = fixture();
    let before = old_settings(dir.path());
    let hash = crate::scaffold::manifest::content_hash(&before);
    let backup = dir.path().join(format!(
        ".magistr/upgrades/configuration/{hash}/.magistr/project.json"
    ));
    std::fs::create_dir_all(backup.parent().unwrap()).unwrap();
    std::fs::write(backup, "different").unwrap();
    assert!(ProjectUpgrade::apply(dir.path()).is_err());
    assert_eq!(
        std::fs::read_to_string(dir.path().join(".magistr/project.json")).unwrap(),
        before
    );
    let invalid = before.replace("think-and-ship", "unknown-backend");
    std::fs::write(dir.path().join(".magistr/project.json"), &invalid).unwrap();
    assert!(crate::scaffold::repair::plan(dir.path()).is_err());
    assert_eq!(
        std::fs::read_to_string(dir.path().join(".magistr/project.json")).unwrap(),
        invalid
    );
}

#[test]
fn custom_transports_stay_custom_and_known_launchers_keep_extra_options() {
    for command in ["wrapper", "bun"] {
        let before = serde_json::json!({"command":command,"args":["mcp-launch","magistr","."],"env":{"KEEP":"yes"}});
        let mut after = before.clone();
        assert!(!ProjectUpgrade::connection(&mut after));
        assert_eq!(after, before);
    }
    let mut entry = serde_json::json!({"command":"/opt/bin/magistr","args":["mcp-launch","magistr","/project with spaces"],"env":{"KEEP":"yes"}});
    assert!(ProjectUpgrade::connection(&mut entry));
    assert_eq!(
        entry["args"],
        serde_json::json!(["mcp", "--dir", "/project with spaces"])
    );
    assert_eq!(entry["env"]["KEEP"], "yes");
}

#[test]
fn initial_setup_adopts_the_saved_identity_without_mutating_its_original() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir(root.join(".think-and-ship")).unwrap();
    let original = r#"{"id":"existing-project"}"#;
    std::fs::write(root.join(".think-and-ship/project.json"), original).unwrap();
    ScaffoldPipeline::headless(crate::types::Stack::default())
        .generate(root)
        .unwrap();
    let document: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join(".magistr/project.json")).unwrap())
            .unwrap();
    assert_eq!(document["project"]["id"], "existing-project");
    assert!(
        root.join(magistr_engine::native::WorkflowUpgrade::REQUEST)
            .is_file()
    );
    assert_eq!(
        std::fs::read_to_string(root.join(".think-and-ship/project.json")).unwrap(),
        original
    );
}

#[test]
fn an_older_declared_identity_is_pinned_with_its_upgrade_request_before_settings() {
    let dir = fixture();
    let root = dir.path();
    let path = root.join(".magistr/project.json");
    let mut document: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    document["project"].as_object_mut().unwrap().remove("id");
    std::fs::write(&path, document.to_string()).unwrap();
    std::fs::create_dir(root.join(".think-and-ship")).unwrap();
    std::fs::write(
        root.join(".think-and-ship/project.json"),
        r#"{"id":"older-project"}"#,
    )
    .unwrap();
    let prepared = crate::scaffold::repair::prepare(
        root,
        &crate::config::inputs::ProjectInputs::from_lookup(root, |_| None),
    )
    .unwrap();
    assert_eq!(
        prepared.changes().files[0].path,
        magistr_engine::native::WorkflowUpgrade::REQUEST
    );
    assert!(
        !root
            .join(magistr_engine::native::WorkflowUpgrade::REQUEST)
            .exists()
    );
    prepared.apply().unwrap();
    let updated: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(updated["project"]["id"], "older-project");
    let request: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.join(magistr_engine::native::WorkflowUpgrade::REQUEST)).unwrap(),
    )
    .unwrap();
    assert_eq!(request["project_id"], "older-project");
    assert_eq!(request["complete"], false);
    assert!(ProjectUpgrade::plan(root).unwrap().files.is_empty());
}
