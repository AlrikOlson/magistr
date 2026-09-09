use super::*;
use crate::types::{Framework, Language, PackageManager};
use magistr_core::types::{
    DetectedProject, ProjectMetadata, ProjectRole, ProjectTree, Topology, Workspace,
};

fn test_stack() -> Stack {
    Stack {
        language: Language::Rust,
        framework: Framework::None,
        package_manager: PackageManager::Cargo,
        project_name: "test-project".to_string(),
        ..Stack::default()
    }
}

#[test]
fn headless_produces_template_vars() {
    let pipeline = ScaffoldPipeline::headless(test_stack());
    let vars = pipeline.template_vars();

    assert_eq!(vars.get("project_name").unwrap(), "test-project");
    assert_eq!(vars.get("language").unwrap(), "rust");
    assert!(!vars.get("check_cmd").unwrap().is_empty());
}

#[test]
fn headless_has_empty_overrides() {
    let pipeline = ScaffoldPipeline::headless(test_stack());
    assert!(pipeline.overrides().is_empty());
}

#[test]
fn with_selections_includes_ai_overrides() {
    let mut wr = ScaffoldSelections::default();
    wr.generated_files
        .insert("CLAUDE.md".to_string(), "# Custom".to_string());
    wr.conventions = vec!["Use Result for errors".to_string()];

    let pipeline = ScaffoldPipeline::with_selections(test_stack(), wr);

    assert_eq!(pipeline.overrides().len(), 1);
    assert_eq!(pipeline.overrides().get("CLAUDE.md").unwrap(), "# Custom");

    let vars = pipeline.template_vars();
    assert!(vars.get("ai_conventions").unwrap().contains("Use Result"));
}

#[test]
fn explicit_targets_reach_preview_for_headless_recommended_and_polyglot_setup() {
    let tmp = tempfile::TempDir::new().unwrap();
    let tree = make_polyglot_tree(tmp.path());
    let selections = ScaffoldSelections {
        selected_agents: vec!["copilot".to_owned()],
        ..ScaffoldSelections::default()
    };
    for pipeline in [
        ScaffoldPipeline::headless(test_stack()),
        ScaffoldPipeline::with_selections(test_stack(), selections),
        ScaffoldPipeline::polyglot(&tree),
    ] {
        let pipeline = pipeline.with_agents(&["codex".to_owned(), "codex".to_owned()]);
        let preview = pipeline.preview(tmp.path()).unwrap();
        assert!(
            preview
                .files
                .iter()
                .any(|file| file.path == ".codex/config.toml")
        );
        assert!(
            preview
                .files
                .iter()
                .any(|file| file.path == ".agents/skills/next/SKILL.md")
        );
        assert!(
            !preview
                .files
                .iter()
                .any(|file| file.path.starts_with(".cursor/"))
        );
        assert!(
            !preview
                .files
                .iter()
                .any(|file| file.path == ".github/copilot-instructions.md")
        );
    }
    assert!(!tmp.path().join(".codex").exists());
}

#[test]
fn omitted_cli_targets_keep_recommended_codex_selection() {
    let tmp = tempfile::TempDir::new().unwrap();
    let selections = ScaffoldSelections {
        selected_agents: vec!["codex".to_owned()],
        ..ScaffoldSelections::default()
    };
    let pipeline = ScaffoldPipeline::with_selections(test_stack(), selections).with_agents(&[]);
    let written = pipeline.generate(tmp.path()).unwrap();
    assert!(written.iter().any(|path| path == ".codex/config.toml"));
    assert!(scaffold::agents_present(tmp.path()).contains(&"codex".to_owned()));
}

#[test]
fn generate_writes_files() {
    let tmp = tempfile::TempDir::new().unwrap();
    let pipeline = ScaffoldPipeline::headless(test_stack());
    let written = pipeline.generate(tmp.path()).unwrap();

    assert!(!written.is_empty());
    assert!(tmp.path().join("CLAUDE.md").exists());
    assert!(tmp.path().join(".magistr/project.json").exists());
}

#[test]
fn preview_does_not_write() {
    let tmp = tempfile::TempDir::new().unwrap();
    let pipeline = ScaffoldPipeline::headless(test_stack());
    let preview = pipeline.preview(tmp.path()).unwrap();

    assert!(!preview.files.is_empty());
    // Preview should not create any files
    assert!(!tmp.path().join("CLAUDE.md").exists());
}

fn make_polyglot_tree(dir: &std::path::Path) -> ProjectTree {
    // Create minimal project markers so detection can work
    let api_dir = dir.join("services/api");
    std::fs::create_dir_all(api_dir.join("src")).unwrap();
    std::fs::write(
        api_dir.join("Cargo.toml"),
        "[package]\nname = \"api\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    std::fs::write(api_dir.join("src/main.rs"), "fn main() {}").unwrap();

    let web_dir = dir.join("apps/web");
    std::fs::create_dir_all(&web_dir).unwrap();
    std::fs::write(
        web_dir.join("package.json"),
        r#"{"name": "web", "dependencies": {"next": "14.0.0"}}"#,
    )
    .unwrap();

    ProjectTree {
        root: dir.to_path_buf(),
        projects: vec![
            DetectedProject {
                path: std::path::PathBuf::from("services/api"),
                language: magistr_core::types::Language::Rust,
                framework: None,
                role: ProjectRole::Backend,
                markers: vec![],
                metadata: ProjectMetadata {
                    name: Some("api".to_string()),
                    dependencies: vec![],
                    version: None,
                },
                parent: None,
                children: vec![],
            },
            DetectedProject {
                path: std::path::PathBuf::from("apps/web"),
                language: magistr_core::types::Language::TypeScript,
                framework: Some(magistr_core::types::Framework::Next),
                role: ProjectRole::Frontend,
                markers: vec![],
                metadata: ProjectMetadata {
                    name: Some("web".to_string()),
                    dependencies: vec![],
                    version: None,
                },
                parent: None,
                children: vec![],
            },
        ],
        edges: vec![],
        workspace: Workspace::None,
        topology: Topology::FlatMonorepo,
    }
}

#[test]
fn polyglot_pipeline_is_polyglot() {
    let tmp = tempfile::TempDir::new().unwrap();
    let tree = make_polyglot_tree(tmp.path());
    let pipeline = ScaffoldPipeline::polyglot(&tree);

    assert!(pipeline.is_polyglot());
}

#[test]
fn polyglot_generate_writes_archetypes() {
    let tmp = tempfile::TempDir::new().unwrap();
    let tree = make_polyglot_tree(tmp.path());
    let pipeline = ScaffoldPipeline::polyglot(&tree);
    pipeline.generate(tmp.path()).unwrap();

    let config_content = std::fs::read_to_string(tmp.path().join(".magistr/project.json")).unwrap();
    let config: crate::types::ProjectConfig = serde_json::from_str(&config_content).unwrap();

    assert_eq!(config.archetypes.len(), 2);
    assert_eq!(config.archetypes[0].id, "api");
    assert_eq!(config.archetypes[1].id, "web");
    assert_eq!(config.topology, Some(Topology::FlatMonorepo));
}

#[test]
fn polyglot_reinit_keeps_configured_archetypes_and_hooks() {
    let tmp = tempfile::TempDir::new().unwrap();
    let tree = make_polyglot_tree(tmp.path());
    let pipeline = ScaffoldPipeline::polyglot(&tree);
    pipeline.generate(tmp.path()).unwrap();
    let path = tmp.path().join(".magistr/project.json");
    let mut config: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    config["archetypes"][0]["quality_gate"]["check"] = "custom api check".into();
    config["lifecycle_hooks"] = serde_json::json!([]);
    config["future"] = serde_json::json!({"keep": true});
    let expected = format!("{}\n", serde_json::to_string_pretty(&config).unwrap());
    std::fs::write(&path, &expected).unwrap();
    let conventions = std::fs::read_to_string(tmp.path().join("CLAUDE.md")).unwrap();

    pipeline.generate(tmp.path()).unwrap();
    assert_eq!(std::fs::read_to_string(path).unwrap(), expected);
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("CLAUDE.md")).unwrap(),
        conventions
    );
}

#[test]
fn polyglot_generate_adds_archetype_section_to_claude_md() {
    let tmp = tempfile::TempDir::new().unwrap();
    let tree = make_polyglot_tree(tmp.path());
    let pipeline = ScaffoldPipeline::polyglot(&tree);
    pipeline.generate(tmp.path()).unwrap();

    let claude_md = std::fs::read_to_string(tmp.path().join("CLAUDE.md")).unwrap();
    assert!(
        claude_md.contains("<!-- archetypes -->"),
        "CLAUDE.md should contain archetype markers"
    );
    assert!(
        claude_md.contains("| api |"),
        "CLAUDE.md should list api archetype"
    );
    assert!(
        claude_md.contains("| web |"),
        "CLAUDE.md should list web archetype"
    );
}

#[test]
fn upsert_marked_section_appends_when_absent() {
    let doc = "# Hello\n\nSome content.\n";
    let result = upsert_marked_section(doc, "<!-- start -->", "<!-- end -->", "New stuff\n");
    assert!(result.contains("<!-- start -->\nNew stuff\n<!-- end -->"));
    assert!(result.contains("# Hello"));
}

#[test]
fn upsert_marked_section_replaces_when_present() {
    let doc = "Before\n<!-- start -->\nOld stuff\n<!-- end -->\nAfter\n";
    let result = upsert_marked_section(doc, "<!-- start -->", "<!-- end -->", "New stuff\n");
    assert!(result.contains("<!-- start -->\nNew stuff\n<!-- end -->"));
    assert!(result.contains("Before"));
    assert!(result.contains("After"));
    assert!(!result.contains("Old stuff"));
}
