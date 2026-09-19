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

const IDENTITY: &str = ".think-and-ship/project.json";

fn read(dir: &std::path::Path, path: &str) -> String {
    std::fs::read_to_string(dir.join(path)).unwrap()
}

/// A first init's plan is complete: the configuration it creates already
/// carries the hooks and the archetypes, the document carries the archetype
/// section beside the think-and-ship one, and the identity declaration is a
/// planned create — and generation writes exactly those bytes.
#[test]
fn a_first_init_plan_carries_the_seeded_config_the_archetype_section_and_the_identity() {
    let tmp = tempfile::TempDir::new().unwrap();
    let tree = make_polyglot_tree(tmp.path());
    let pipeline = ScaffoldPipeline::polyglot(&tree);

    let plan = pipeline.preview(tmp.path()).unwrap();
    let config = plan.get(".magistr/project.json").unwrap();
    assert_eq!(config.action, scaffold::FileAction::Create);
    let parsed: crate::types::ProjectConfig = serde_json::from_str(&config.new_content).unwrap();
    assert!(
        !parsed.lifecycle_hooks.is_empty(),
        "preset hooks are planted"
    );
    assert_eq!(parsed.archetypes.len(), 2);
    assert_eq!(parsed.topology, Some(Topology::FlatMonorepo));

    let claude = plan.get("CLAUDE.md").unwrap();
    assert_eq!(claude.action, scaffold::FileAction::Create);
    assert!(claude.new_content.contains("<!-- think-and-ship -->"));
    assert!(claude.new_content.contains("<!-- archetypes -->"));
    assert!(claude.new_content.contains("| api |"));
    assert!(claude.new_content.contains("| web |"));

    let identity = plan.get(IDENTITY).unwrap();
    assert_eq!(identity.action, scaffold::FileAction::Create);
    assert_eq!(identity.ownership, scaffold::Ownership::Unclaimed);
    let declared: serde_json::Value = serde_json::from_str(&identity.new_content).unwrap();
    assert_eq!(
        declared["id"],
        magistr_engine::native::project_id_for_path(&tmp.path().canonicalize().unwrap())
    );
    assert!(!tmp.path().join(".think-and-ship").exists());
    assert!(!tmp.path().join(".magistr").exists());

    let written = pipeline.generate(tmp.path()).unwrap();
    assert!(written.iter().any(|path| path == IDENTITY));
    for change in [config, claude, identity] {
        assert_eq!(
            read(tmp.path(), &change.path),
            change.new_content,
            "{}",
            change.path
        );
    }
}

/// A second plan preserves the configuration and the identity the first
/// wrote: they are the user's now, and re-detection never rewrites them.
#[test]
fn a_second_plan_preserves_the_configuration_and_the_identity() {
    let tmp = tempfile::TempDir::new().unwrap();
    let tree = make_polyglot_tree(tmp.path());
    let pipeline = ScaffoldPipeline::polyglot(&tree);
    pipeline.generate(tmp.path()).unwrap();
    let before =
        [".magistr/project.json", "CLAUDE.md", IDENTITY].map(|path| read(tmp.path(), path));

    let plan = pipeline.preview(tmp.path()).unwrap();
    let config = plan.get(".magistr/project.json").unwrap();
    assert_eq!(config.action, scaffold::FileAction::Preserved);
    assert_eq!(config.ownership, scaffold::Ownership::User);
    let identity = plan.get(IDENTITY).unwrap();
    assert_eq!(identity.action, scaffold::FileAction::Preserved);
    assert_eq!(identity.ownership, scaffold::Ownership::User);
    assert_eq!(identity.new_content, before[2]);
    assert_eq!(
        plan.get("CLAUDE.md").unwrap().action,
        scaffold::FileAction::Unchanged
    );
    assert_eq!(
        plan.applies(),
        0,
        "{:?}",
        plan.applied().map(|c| &c.path).collect::<Vec<_>>()
    );

    pipeline.generate(tmp.path()).unwrap();
    let after = [".magistr/project.json", "CLAUDE.md", IDENTITY].map(|path| read(tmp.path(), path));
    assert_eq!(after, before);
}

/// An identity an ancestor declares is the project's identity already; the
/// plan declares nothing at the root and generation writes nothing there.
#[test]
fn an_identity_declared_by_an_ancestor_plans_nothing_at_the_root() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join(".think-and-ship")).unwrap();
    std::fs::write(tmp.path().join(IDENTITY), "{\"id\":\"outer-a1b2c3\"}\n").unwrap();
    let root = tmp.path().join("nested");
    std::fs::create_dir_all(&root).unwrap();
    let pipeline = ScaffoldPipeline::headless(test_stack());

    let plan = pipeline.preview(&root).unwrap();
    assert!(plan.get(IDENTITY).is_none());
    pipeline.generate(&root).unwrap();
    assert!(!root.join(".think-and-ship").exists());
    assert_eq!(read(tmp.path(), IDENTITY), "{\"id\":\"outer-a1b2c3\"}\n");
}

/// A headless single-stack init plants the language preset's hooks.
#[test]
fn a_headless_first_init_plans_the_language_preset_hooks() {
    let tmp = tempfile::TempDir::new().unwrap();
    let pipeline = ScaffoldPipeline::headless(test_stack());
    let plan = pipeline.preview(tmp.path()).unwrap();
    let config = plan.get(".magistr/project.json").unwrap();
    let parsed: crate::types::ProjectConfig = serde_json::from_str(&config.new_content).unwrap();
    assert_eq!(
        parsed.lifecycle_hooks,
        magistr_engine::workflow::hook_presets::preset_for_language("rust")
    );
    assert!(parsed.archetypes.is_empty());
    assert!(
        !plan
            .get("CLAUDE.md")
            .unwrap()
            .new_content
            .contains("<!-- archetypes -->")
    );
}
