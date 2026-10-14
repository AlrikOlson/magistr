//! Reinitialization uses the same ownership decisions as generation and preview.
use std::path::Path;

use magistr::commands::{config, init};
use magistr::detect::detect_stack;
use magistr::scaffold::{FileAction, pipeline::ScaffoldPipeline};
use tempfile::TempDir;

fn write(dir: &Path, path: &str, content: &str) {
    let full = dir.join(path);
    std::fs::create_dir_all(full.parent().unwrap()).unwrap();
    std::fs::write(full, content).unwrap();
}

fn fixture() -> TempDir {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "Cargo.toml",
        "[package]\nname = \"reinit\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    );
    let pipeline = ScaffoldPipeline::headless(detect_stack(tmp.path()).unwrap());
    pipeline.generate(tmp.path()).unwrap();
    tmp
}

fn read(dir: &Path, path: &str) -> String {
    std::fs::read_to_string(dir.join(path)).unwrap()
}

#[tokio::test]
async fn force_reinit_preserves_state_config_and_customized_scaffold() {
    let tmp = fixture();
    let dir = tmp.path();
    let mut config: serde_json::Value =
        serde_json::from_str(&read(dir, ".magistr/project.json")).unwrap();
    config["quality_gate"] = serde_json::json!({"check":"echo configured > configured-check.txt","test":"true","lint":"true"});
    config["custom_gates"] = serde_json::json!([{"name":"mine","command":"true","required":true}]);
    config["future_setting"] = serde_json::json!({"keep":42});
    config["lifecycle_hooks"] = serde_json::json!([]);
    let project = format!("{}\n", serde_json::to_string_pretty(&config).unwrap());
    write(dir, ".magistr/project.json", &project);
    let state_files = [
        (".magistr/state.json", "{\"phase\":\"execute\"}"),
        (".magistr/iteration-log.md", "earlier receipt\n"),
        (".magistr/session-state.json", "{\"saved\":true}"),
        (".claude/skills/mine/SKILL.md", "custom skill\n"),
        (".claude/rules/conventions.md", "our conventions\n"),
    ];
    for (path, content) in state_files {
        write(dir, path, content);
    }
    let identity = read(dir, ".think-and-ship/project.json");
    let manifest = read(dir, ".magistr/scaffold-manifest.json");
    let mut manifest_value: serde_json::Value = serde_json::from_str(&manifest).unwrap();
    manifest_value["files"][".claude/retired.md"] = "older-hash".into();
    write(
        dir,
        ".magistr/scaffold-manifest.json",
        &manifest_value.to_string(),
    );
    write(dir, "tools/mcp-launcher", "local executable");
    let custom_mcp =
        serde_json::json!({"command":"tools/mcp-launcher","args":["mcp-serve","--project","."]});
    let mut mcp: serde_json::Value = serde_json::from_str(&read(dir, ".mcp.json")).unwrap();
    mcp["mcpServers"]["magistr"] = custom_mcp.clone();
    write(dir, ".mcp.json", &mcp.to_string());
    std::fs::remove_file(dir.join(".claude/skills/next/SKILL.md")).unwrap();

    // A capture with no home: the interview composes built-in layers only.
    let inputs = magistr::config::inputs::ProjectInputs::from_lookup(dir, |_| None);
    for _ in 0..2 {
        init::cmd_init_interview(dir, true, true, &[], &inputs)
            .await
            .unwrap();
        assert_eq!(
            read(dir, "configured-check.txt").trim(),
            "configured",
            "post-init checks use the preserved commands"
        );
        assert_eq!(
            read(dir, ".magistr/project.json"),
            project,
            "configured commands and unknown keys"
        );
        for (path, content) in state_files {
            assert_eq!(read(dir, path), content, "{path}");
        }
        assert_eq!(read(dir, ".think-and-ship/project.json"), identity);
        let after: serde_json::Value =
            serde_json::from_str(&read(dir, ".magistr/scaffold-manifest.json")).unwrap();
        assert_eq!(
            after["files"][".claude/retired.md"], "older-hash",
            "old ownership evidence survives"
        );
        assert_eq!(
            after["files"][".claude/rules/conventions.md"],
            manifest_value["files"][".claude/rules/conventions.md"],
            "customization baseline survives"
        );
        let after: serde_json::Value = serde_json::from_str(&read(dir, ".mcp.json")).unwrap();
        assert_eq!(after["mcpServers"]["magistr"], custom_mcp);
        assert!(
            dir.join(".claude/skills/next/SKILL.md").is_file(),
            "missing generated skill is repaired"
        );
    }
}

#[test]
fn reinit_preview_and_generation_preserve_existing_project_bytes() {
    let tmp = fixture();
    let dir = tmp.path();
    let mut config: serde_json::Value =
        serde_json::from_str(&read(dir, ".magistr/project.json")).unwrap();
    config["future_setting"] = 42.into();
    config["quality_gate"]["check"] = "custom check".into();
    let project = format!("{}\n", serde_json::to_string_pretty(&config).unwrap());
    write(dir, ".magistr/project.json", &project);
    let pipeline = ScaffoldPipeline::headless(detect_stack(dir).unwrap());
    let preview = pipeline.preview(dir).unwrap();
    let file = preview
        .files
        .iter()
        .find(|f| f.path == ".magistr/project.json")
        .unwrap();
    assert_eq!(file.action, FileAction::Preserved);
    assert_eq!(file.new_content, project);
    assert!(file.diff.is_none());
    pipeline.generate(dir).unwrap();
    assert_eq!(read(dir, ".magistr/project.json"), project);
}

#[test]
fn reinit_preserves_explicit_mcp_overrides_in_preview_and_generation() {
    for entry in [
        serde_json::json!({"command":"my-workbench","args":["--workspace","."]}),
        serde_json::json!({"command":"/not/installed/yet/workbench","args":["."]}),
        serde_json::json!({"command":"workbench-mcp","args":["."],"env":{"CUSTOM":"keep"}}),
        serde_json::json!({"type":"http","url":"https://example.invalid/mcp"}),
    ] {
        let tmp = fixture();
        let dir = tmp.path();
        let mut mcp: serde_json::Value = serde_json::from_str(&read(dir, ".mcp.json")).unwrap();
        mcp["mcpServers"]["workbench-mcp"] = entry.clone();
        write(dir, ".mcp.json", &mcp.to_string());
        let pipeline = ScaffoldPipeline::headless(detect_stack(dir).unwrap());
        let preview = pipeline.preview(dir).unwrap();
        let file = preview
            .files
            .iter()
            .find(|f| f.path == ".mcp.json")
            .unwrap();
        let preview_doc: serde_json::Value = serde_json::from_str(&file.new_content).unwrap();
        assert_eq!(preview_doc["mcpServers"]["workbench-mcp"], entry);
        pipeline.generate(dir).unwrap();
        let after: serde_json::Value = serde_json::from_str(&read(dir, ".mcp.json")).unwrap();
        assert_eq!(after["mcpServers"]["workbench-mcp"], entry);
    }
}

#[test]
fn reinit_rejects_invalid_json_before_any_scaffold_write() {
    for (path, invalid) in [
        (".magistr/project.json", "{broken"),
        (".magistr/project.json", "[]"),
        (".mcp.json", "{broken"),
        (".mcp.json", "null"),
        (".mcp.json", "{\"mcpServers\":[]}"),
    ] {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();
        write(dir, path, invalid);
        let pipeline = ScaffoldPipeline::headless(detect_stack(dir).unwrap());
        let before: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        for result in [
            pipeline.preview(dir).map(|_| ()),
            pipeline.generate(dir).map(|_| ()),
        ] {
            let error = result.expect_err(path).to_string().replace('\\', "/");
            assert!(error.contains(path), "{error}");
        }
        let after: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(after, before, "preflight must precede writes: {path}");
        assert_eq!(read(dir, path), invalid);
        assert!(!dir.join(".magistr/scaffold-manifest.json").exists());
    }
}

#[test]
fn reinit_rejects_nonregular_config_paths_without_writes() {
    for path in [".magistr/project.json", ".mcp.json"] {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(path)).unwrap();
        let pipeline = ScaffoldPipeline::headless(detect_stack(tmp.path()).unwrap());
        assert!(pipeline.preview(tmp.path()).is_err());
        assert!(pipeline.generate(tmp.path()).is_err());
        assert!(tmp.path().join(path).is_dir());
        assert!(!tmp.path().join(".claude").exists());
    }
}

#[cfg(unix)]
#[test]
fn reinit_rejects_symlinked_config_and_namespace() {
    use std::os::unix::fs::symlink;
    for path in [".magistr", ".magistr/project.json", ".mcp.json"] {
        let tmp = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        write(outside.path(), "project.json", "{\"keep\":true}");
        let destination = if path == ".magistr" {
            outside.path().to_path_buf()
        } else {
            outside.path().join("project.json")
        };
        let link = tmp.path().join(path);
        std::fs::create_dir_all(link.parent().unwrap()).unwrap();
        symlink(&destination, &link).unwrap();
        let pipeline = ScaffoldPipeline::headless(detect_stack(tmp.path()).unwrap());
        assert!(pipeline.preview(tmp.path()).is_err());
        assert!(pipeline.generate(tmp.path()).is_err());
        assert_eq!(read(outside.path(), "project.json"), "{\"keep\":true}");
        assert!(std::fs::symlink_metadata(link).unwrap().is_symlink());
        assert!(!tmp.path().join(".claude").exists());
    }
}

#[test]
fn force_reinit_with_missing_preset_preserves_the_existing_setup() {
    let tmp = fixture();
    let dir = tmp.path();
    let project = read(dir, ".magistr/project.json");
    let manifest = read(dir, ".magistr/scaffold-manifest.json");
    for result in [
        config::cmd_import(dir, "missing-preset.toml", true),
        config::cmd_init_from_preset(dir, "missing-preset.toml", true, &[]),
    ] {
        assert!(result.is_err());
        assert_eq!(read(dir, ".magistr/project.json"), project);
        assert_eq!(read(dir, ".magistr/scaffold-manifest.json"), manifest);
        assert!(dir.join(".claude/skills/next/SKILL.md").is_file());
    }
}
