use super::*;

use magistr_core::types::{
    DetectedProject, EdgeKind, Framework, Language, ProjectEdge, ProjectMetadata, ProjectRole,
    ProjectTree, Topology, Workspace,
};
use std::path::PathBuf;

fn rust_project(path: &str, name: &str) -> DetectedProject {
    DetectedProject {
        path: PathBuf::from(path),
        language: Language::Rust,
        framework: None,
        role: ProjectRole::Backend,
        markers: vec![],
        metadata: ProjectMetadata {
            name: Some(name.to_string()),
            dependencies: vec![],
            version: None,
        },
        parent: None,
        children: vec![],
    }
}

fn ts_project(path: &str, name: &str, framework: Framework) -> DetectedProject {
    DetectedProject {
        path: PathBuf::from(path),
        language: Language::TypeScript,
        framework: Some(framework),
        role: ProjectRole::Frontend,
        markers: vec![],
        metadata: ProjectMetadata {
            name: Some(name.to_string()),
            dependencies: vec![],
            version: None,
        },
        parent: None,
        children: vec![],
    }
}

fn make_tree(projects: Vec<DetectedProject>, edges: Vec<ProjectEdge>) -> ProjectTree {
    ProjectTree {
        root: PathBuf::from("/tmp/test-repo"),
        projects,
        edges,
        workspace: Workspace::None,
        topology: Topology::SingleProject,
    }
}

#[test]
fn single_project_produces_one_archetype() {
    let tree = make_tree(vec![rust_project(".", "my-cli")], vec![]);
    let result = tree_to_archetypes(&tree, None);

    assert_eq!(result.archetypes.len(), 1);
    assert_eq!(result.archetypes[0].id, "my-cli");
    assert_eq!(result.archetypes[0].language, "rust");
    assert!(result.archetypes[0].framework.is_none());
    assert_eq!(result.archetypes[0].role, ProjectRole::Backend);
    assert_eq!(result.archetypes[0].root, PathBuf::from("."));
    assert!(result.archetypes[0].quality_gate.check.is_empty());
}

#[test]
fn polyglot_produces_multiple_archetypes() {
    let tree = make_tree(
        vec![
            rust_project("services/api", "api"),
            ts_project("apps/web", "web", Framework::Next),
        ],
        vec![],
    );
    let result = tree_to_archetypes(&tree, None);

    assert_eq!(result.archetypes.len(), 2);
    assert_eq!(result.archetypes[0].id, "api");
    assert_eq!(result.archetypes[0].language, "rust");
    assert_eq!(result.archetypes[1].id, "web");
    assert_eq!(result.archetypes[1].language, "typescript");
    assert_eq!(result.archetypes[1].framework.as_deref(), Some("next"));
}

#[test]
fn edges_produce_relationships() {
    let tree = make_tree(
        vec![
            ts_project("apps/web", "web", Framework::Next),
            rust_project("services/api", "api"),
        ],
        vec![ProjectEdge {
            from: 0,
            to: 1,
            kind: EdgeKind::NpmWorkspaceRef,
        }],
    );
    let result = tree_to_archetypes(&tree, None);

    assert_eq!(result.relationships.len(), 1);
    assert_eq!(result.relationships[0].from, "web");
    assert_eq!(result.relationships[0].to, "api");
}

#[test]
fn topology_is_preserved() {
    let mut tree = make_tree(
        vec![
            rust_project("crates/core", "core"),
            rust_project("crates/cli", "cli"),
        ],
        vec![],
    );
    tree.topology = Topology::FlatMonorepo;
    tree.workspace = Workspace::Cargo;

    let result = tree_to_archetypes(&tree, None);
    assert_eq!(result.topology, Topology::FlatMonorepo);
}

#[test]
fn unnamed_project_derives_id_from_path() {
    let mut project = rust_project("packages/shared-lib", "");
    project.metadata.name = None;
    let tree = make_tree(vec![project], vec![]);
    let result = tree_to_archetypes(&tree, None);

    assert_eq!(result.archetypes[0].id, "shared-lib");
    assert_eq!(result.archetypes[0].name, "shared-lib");
}

#[test]
fn root_project_with_no_name_uses_default() {
    let mut project = rust_project(".", "");
    project.metadata.name = None;
    let tree = make_tree(vec![project], vec![]);
    let result = tree_to_archetypes(&tree, None);

    assert_eq!(result.archetypes[0].id, "default");
}

#[test]
fn manifest_populates_builtin_quality_gate() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("Cargo.toml"),
        "[package]\nname=\"my-app\"\nversion=\"0.1.0\"",
    )
    .unwrap();
    let mut tree = make_tree(vec![rust_project(".", "my-app")], vec![]);
    tree.root = tmp.path().into();
    let result = tree_to_archetypes(&tree, None);

    let qg = &result.archetypes[0].quality_gate;
    assert!(
        qg.check.contains("cargo"),
        "check should use cargo: {}",
        qg.check
    );
    assert!(
        qg.test.contains("cargo") || qg.test.contains("nextest"),
        "test should use cargo: {}",
        qg.test
    );
    assert!(
        qg.lint.contains("clippy"),
        "lint should use clippy: {}",
        qg.lint
    );
}

#[test]
fn typescript_without_tool_evidence_leaves_format_unconfigured() {
    let tree = make_tree(vec![ts_project(".", "web-app", Framework::Next)], vec![]);
    let result = tree_to_archetypes(&tree, None);

    let fmt = &result.archetypes[0].format;
    assert!(fmt.command.is_empty(), "no formatter was observed");
}

#[test]
fn duplicate_ids_get_suffixed() {
    let tree = make_tree(
        vec![
            rust_project("a/core", "core"),
            rust_project("b/core", "core"),
        ],
        vec![],
    );
    let result = tree_to_archetypes(&tree, None);

    let ids: Vec<&str> = result.archetypes.iter().map(|a| a.id.as_str()).collect();
    assert_ne!(ids[0], ids[1], "ids must be unique: {:?}", ids);
}

#[test]
fn dual_stack_root_gets_language_suffixed_id_and_real_commands() {
    // A polyglot root: the rust twin and the ts twin share a path and a name.
    // The repo really uses bun (bun.lock) and biome (biome.json).
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("bun.lock"), "").unwrap();
    std::fs::write(tmp.path().join("biome.json"), "{}").unwrap();
    std::fs::write(
        tmp.path().join("package.json"),
        "{\"name\": \"myapp\", \"devDependencies\": {\"typescript\": \"^5\"}}",
    )
    .unwrap();

    let mut tree = make_tree(
        vec![rust_project(".", "myapp"), {
            let mut p = ts_project(".", "myapp", Framework::Next);
            p.framework = None;
            p
        }],
        vec![],
    );
    tree.root = tmp.path().to_path_buf();

    let result = tree_to_archetypes(&tree, None);

    assert_eq!(result.archetypes.len(), 2);
    assert_eq!(result.archetypes[0].id, "myapp");
    assert_eq!(result.archetypes[0].language, "rust");
    // The twin is distinguished by its language, not a numeric suffix.
    assert_eq!(result.archetypes[1].id, "myapp-typescript");
    // Real commands: the repo's package manager is bun, not npm's defaults…
    assert!(result.archetypes[1].quality_gate.test.is_empty());
    assert_eq!(
        result.archetypes[1].quality_gate.lint,
        "./node_modules/.bin/biome check ."
    );
    // …and its formatter is biome — handing it prettier would rewrite the repo.
    assert!(
        result.archetypes[1].format.command.contains("biome"),
        "biome repo must not be handed prettier: {}",
        result.archetypes[1].format.command
    );
}

#[test]
fn a_repo_without_tool_evidence_leaves_commands_unconfigured() {
    let tree = make_tree(
        vec![{
            let mut p = ts_project("apps/web", "web", Framework::Next);
            p.framework = None;
            p
        }],
        vec![],
    );
    let result = tree_to_archetypes(&tree, None);
    assert!(result.archetypes[0].quality_gate.test.is_empty());
    assert!(result.archetypes[0].format.command.is_empty());
}
