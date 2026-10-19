use std::io::Cursor;
use std::path::PathBuf;

use magistr_core::types::{DetectedProject, ProjectMetadata, ProjectRole, ProjectTree, Topology};

use super::*;
use crate::types::*;

fn sample_stack() -> Stack {
    Stack {
        project_name: "test-project".into(),
        language: Language::Rust,
        framework: Framework::None,
        package_manager: PackageManager::Cargo,
        test_framework: TestFramework::CargoTest,
        linter: Linter::Clippy,
        formatter: Formatter::Rustfmt,
        workspace: Workspace::None,
        ci_provider: CiProvider::None,
        extra_detections: Vec::new(),
    }
}

fn sample_preset() -> Preset {
    crate::config::preset::fallback_preset()
}

fn project(path: &str, language: Language) -> DetectedProject {
    DetectedProject {
        path: PathBuf::from(path),
        language,
        framework: None,
        role: ProjectRole::Backend,
        markers: Vec::new(),
        metadata: ProjectMetadata::default(),
        parent: None,
        children: Vec::new(),
    }
}

fn sample_tree() -> ProjectTree {
    ProjectTree {
        root: PathBuf::from("/repo"),
        projects: vec![
            project("api", Language::Rust),
            project("web", Language::TypeScript),
        ],
        edges: Vec::new(),
        workspace: Workspace::None,
        topology: Topology::FlatMonorepo,
    }
}

// ── Defaults ───────────────────────────────────────────────────────────

#[test]
fn default_selects_codex_and_claude() {
    let result = InterviewResult::default();
    assert_eq!(result.selected_agents, vec!["codex", "claude"]);
    assert!(result.include_hooks);
    assert!(result.quality_gate.is_none());
    assert_eq!(result.project_type, "cli");
}

#[test]
fn defaults_do_not_install_external_connections() {
    assert!(
        InterviewResult::defaults_for(&sample_stack())
            .selected_mcp_servers
            .is_empty()
    );
}

#[test]
fn all_agents_have_unique_ids() {
    let mut ids: Vec<&str> = ALL_AGENTS.iter().map(|a| a.id).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), ALL_AGENTS.len());
}

#[test]
fn codex_can_be_selected_without_claude() {
    let script = "\n\n\n2\n\n\n\n";
    let mut reader = Cursor::new(script.as_bytes().to_vec());
    let result = run_interview(&mut reader, &sample_stack(), &sample_preset()).unwrap();
    assert_eq!(result.selected_agents, vec!["codex"]);
}

#[test]
fn to_selections_carries_user_choices() {
    let result = InterviewResult {
        project_type: "api".into(),
        description: "My API".into(),
        selected_agents: vec!["cursor".into(), "copilot".into()],
        selected_mcp_servers: vec!["context7".into()],
        quality_gate: Some(RecommendedQualityGate {
            check: Some("cargo check".into()),
            test: None,
            lint: None,
        }),
        include_hooks: false,
    };
    let sel = result.to_selections();
    assert_eq!(sel.project_type, "api");
    assert_eq!(sel.description, "My API");
    assert_eq!(sel.selected_agents, vec!["cursor", "copilot"]);
    assert_eq!(sel.selected_mcp_servers, vec!["context7"]);
    assert_eq!(
        sel.quality_gate.as_ref().and_then(|g| g.check.as_deref()),
        Some("cargo check")
    );
    // AI-only fields stay at their defaults.
    assert!(sel.conventions.is_empty());
    assert!(sel.generated_files.is_empty());
}

// ── Scripted interview runs ────────────────────────────────────────────

#[test]
fn all_enter_answers_equal_the_yes_defaults() {
    // Enough blank lines to answer every prompt with its default.
    let mut reader = Cursor::new(b"\n\n\n\n\n\n\n\n\n\n".to_vec());
    let stack = sample_stack();
    let interviewed =
        run_interview(&mut reader, &stack, &sample_preset()).expect("interview completes");
    assert_eq!(interviewed, InterviewResult::defaults_for(&stack));
}

#[test]
fn eof_answers_equal_the_yes_defaults() {
    // A closed stdin (CI without --yes) must degrade to the default answers.
    let mut reader = Cursor::new(Vec::new());
    let stack = sample_stack();
    let interviewed =
        run_interview(&mut reader, &stack, &sample_preset()).expect("interview completes");
    assert_eq!(interviewed, InterviewResult::defaults_for(&stack));
}

#[test]
fn declining_the_stack_cancels() {
    let mut reader = Cursor::new(b"n\n".to_vec());
    let err = run_interview(&mut reader, &sample_stack(), &sample_preset())
        .expect_err("declining the stack cancels the interview");
    assert!(
        matches!(err, crate::error::MagistrError::Cancelled(_)),
        "expected Cancelled, got {err:?}"
    );
}

#[test]
fn custom_answers_flow_into_the_result() {
    // stack yes, project type 3 (api), description, agents +copilot (toggle 2),
    // mcp defaults, hooks no, customize gates yes, keep check, override test,
    // keep lint, confirm.
    let script = "\n3\nMy API\n5\n\nn\ny\n\ncargo nextest run\n\n\n";
    let mut reader = Cursor::new(script.as_bytes().to_vec());
    let stack = sample_stack();
    let result = run_interview(&mut reader, &stack, &sample_preset()).expect("interview completes");

    assert_eq!(result.project_type, "api");
    assert_eq!(result.description, "My API");
    assert!(result.selected_agents.contains(&"codex".to_string()));
    assert!(result.selected_agents.contains(&"copilot".to_string()));
    assert!(!result.include_hooks);
    let gate = result.quality_gate.expect("customized gate");
    assert_eq!(
        gate.check.as_deref(),
        Some(sample_preset().quality_gate.check.as_str())
    );
    assert_eq!(gate.test.as_deref(), Some("cargo nextest run"));
}

// ── Polyglot tree confirmation ─────────────────────────────────────────

#[test]
fn tree_confirmation_enter_keeps_all_projects() {
    let tree = sample_tree();
    let mut reader = Cursor::new(b"\n".to_vec());
    let confirmed = confirm_project_tree(&mut reader, &tree)
        .expect("confirmation runs")
        .expect("tree kept");
    assert_eq!(confirmed.projects.len(), 2);
}

#[test]
fn tree_confirmation_excludes_by_number() {
    let tree = sample_tree();
    let mut reader = Cursor::new(b"2\n".to_vec());
    let confirmed = confirm_project_tree(&mut reader, &tree)
        .expect("confirmation runs")
        .expect("tree kept");
    assert_eq!(confirmed.projects.len(), 1);
    assert_eq!(confirmed.projects[0].path, PathBuf::from("api"));
}

#[test]
fn tree_confirmation_excluding_everything_returns_none() {
    let tree = sample_tree();
    let mut reader = Cursor::new(b"1,2\n".to_vec());
    let confirmed = confirm_project_tree(&mut reader, &tree).expect("confirmation runs");
    assert!(confirmed.is_none());
}

// ── MCP entry generation ───────────────────────────────────────────────

#[test]
fn build_extra_mcp_entries_resolves_known_servers() {
    let entries =
        build_extra_mcp_entries(&["context7".into(), "memory".into(), "nonexistent".into()]);
    assert!(entries.contains_key("context7"));
    assert!(entries.contains_key("memory"));
    assert!(!entries.contains_key("nonexistent"));
    assert_eq!(entries["context7"]["command"], "npx");
}

// ── Golden fixtures: interview-with-defaults ≡ --yes defaults ──────────
//
// Three fixture repos (rust, node, polyglot). For each, the interview with
// every prompt answered by Enter must write the same .magistr/project.json
// and file list as the non-interactive defaults path (`magistr init --yes`),
// which in turn carries the old wizard's default selections (cursor agent,
// default-enabled MCP servers, preset hooks and gates).

fn write_fixture(root: &std::path::Path, files: &[(&str, &str)]) {
    for (name, content) in files {
        let path = root.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("fixture dir");
        }
        std::fs::write(path, content).expect("fixture file");
    }
}

const RUST_FIXTURE: &[(&str, &str)] = &[
    (
        "Cargo.toml",
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    ),
    ("src/main.rs", "fn main() {}\n"),
];

const NODE_FIXTURE: &[(&str, &str)] = &[
    (
        "package.json",
        r#"{"name": "app", "version": "1.0.0", "scripts": {"test": "vitest"}}"#,
    ),
    ("index.js", "console.log('hi');\n"),
];

const POLYGLOT_FIXTURE: &[(&str, &str)] = &[
    (
        "api/Cargo.toml",
        "[package]\nname = \"api\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    ),
    ("api/src/main.rs", "fn main() {}\n"),
    (
        "web/package.json",
        r#"{"name": "web", "version": "1.0.0", "dependencies": {"react": "^18.0.0"}}"#,
    ),
    ("web/index.jsx", "export default () => null;\n"),
];

/// Generate a single-stack scaffold from an `InterviewResult` the way
/// `cmd_init_interview` does, returning the sorted written-file list and
/// the project.json content.
fn generate_single(fixture: &[(&str, &str)], result: &InterviewResult) -> (Vec<String>, String) {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let dir = tmp.path().join("app");
    write_fixture(&dir, fixture);
    let stack = crate::detect::detect_stack(&dir).expect("detect");
    let pipeline =
        crate::scaffold::pipeline::ScaffoldPipeline::with_selections(stack, result.to_selections());
    let mut written = pipeline.generate(&dir).expect("generate");
    written.sort();
    let json = std::fs::read_to_string(dir.join(".magistr/project.json")).expect("project.json");
    (written, json)
}

fn scripted_defaults_interview(dir: &std::path::Path) -> InterviewResult {
    let stack = crate::detect::detect_stack(dir).expect("detect");
    let preset = crate::config::preset::load_preset_layered(
        stack.language,
        stack.framework,
        stack.package_manager,
        &crate::config::preset::LayerOptions::default(),
    )
    .unwrap_or_else(|_| crate::config::preset::fallback_preset());
    let mut reader = Cursor::new(Vec::new());
    run_interview(&mut reader, &stack, &preset).expect("interview completes")
}

#[test]
fn golden_rust_interview_defaults_match_yes() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let dir = tmp.path().join("app");
    write_fixture(&dir, RUST_FIXTURE);
    let interviewed = scripted_defaults_interview(&dir);
    let stack = crate::detect::detect_stack(&dir).expect("detect");
    assert_eq!(interviewed, InterviewResult::defaults_for(&stack));

    let (files_a, json_a) = generate_single(RUST_FIXTURE, &interviewed);
    let (files_b, json_b) = generate_single(RUST_FIXTURE, &InterviewResult::defaults_for(&stack));
    assert_eq!(files_a, files_b);
    assert_eq!(json_a, json_b);

    // Wizard-default parity: the cursor agent config and a default-enabled
    // MCP server are part of the scaffold.
    assert!(
        files_a.iter().any(|f| f.contains(".codex")),
        "Codex agent config expected in {files_a:?}"
    );
    assert!(json_a.contains("\"language\""));
}

#[test]
fn golden_node_interview_defaults_match_yes() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let dir = tmp.path().join("app");
    write_fixture(&dir, NODE_FIXTURE);
    let interviewed = scripted_defaults_interview(&dir);
    let stack = crate::detect::detect_stack(&dir).expect("detect");
    assert_eq!(interviewed, InterviewResult::defaults_for(&stack));

    let (files_a, json_a) = generate_single(NODE_FIXTURE, &interviewed);
    let (files_b, json_b) = generate_single(NODE_FIXTURE, &InterviewResult::defaults_for(&stack));
    assert_eq!(files_a, files_b);
    assert_eq!(json_a, json_b);
}

#[test]
fn golden_polyglot_tree_confirmation_and_defaults() {
    fn generate_polyglot(result_from_interview: bool) -> (Vec<String>, String) {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let dir = tmp.path().join("app");
        write_fixture(&dir, POLYGLOT_FIXTURE);
        let tree = crate::detect::build_project_tree(&dir).expect("tree");
        assert!(tree.len() > 1, "fixture should detect as polyglot");

        // The polyglot primary's stack drives the interview.
        let primary = tree.projects.first().expect("primary project");
        let primary_stack =
            crate::detect::detect_stack(&tree.root.join(&primary.path)).unwrap_or_default();

        let (tree, result) = if result_from_interview {
            let mut reader = Cursor::new(b"\n".to_vec());
            let confirmed = confirm_project_tree(&mut reader, &tree)
                .expect("confirmation runs")
                .expect("tree kept");
            assert_eq!(confirmed.projects.len(), tree.projects.len());
            let mut reader = Cursor::new(Vec::new());
            let preset = crate::config::preset::fallback_preset();
            let result =
                run_interview(&mut reader, &primary_stack, &preset).expect("interview completes");
            (confirmed, result)
        } else {
            (tree, InterviewResult::defaults_for(&primary_stack))
        };

        let mut pipeline = crate::scaffold::pipeline::ScaffoldPipeline::polyglot(&tree);
        pipeline.selections = result.to_selections();
        let mut written = pipeline.generate(&dir).expect("generate");
        written.sort();
        let json =
            std::fs::read_to_string(dir.join(".magistr/project.json")).expect("project.json");
        (written, json)
    }

    let (files_a, json_a) = generate_polyglot(true);
    let (files_b, json_b) = generate_polyglot(false);
    assert_eq!(files_a, files_b);
    assert_eq!(json_a, json_b);
    assert!(
        json_a.contains("archetypes"),
        "polyglot config should carry archetypes"
    );
}
