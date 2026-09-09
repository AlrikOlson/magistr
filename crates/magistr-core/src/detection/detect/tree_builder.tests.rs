use super::*;
use crate::types::{ProjectRole, Topology};
use std::fs;

fn make_repo() -> tempfile::TempDir {
    let tmp = tempfile::TempDir::new().unwrap();
    fs::create_dir(tmp.path().join(".git")).unwrap();
    tmp
}

#[test]
fn single_rust_project() {
    let tmp = make_repo();
    fs::write(
        tmp.path().join("Cargo.toml"),
        "[package]\nname = \"myapp\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    fs::create_dir_all(tmp.path().join("src")).unwrap();
    fs::write(tmp.path().join("src/main.rs"), "fn main() {}").unwrap();

    let tree = build_project_tree(tmp.path()).unwrap();
    assert_eq!(tree.len(), 1);
    assert_eq!(tree.topology, Topology::SingleProject);
    assert_eq!(tree.projects[0].language, crate::types::Language::Rust);
}

#[test]
fn cargo_workspace() {
    let tmp = make_repo();

    // Root Cargo.toml with workspace
    fs::write(
        tmp.path().join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/core\", \"crates/cli\"]\n",
    )
    .unwrap();

    // Member crates
    fs::create_dir_all(tmp.path().join("crates/core/src")).unwrap();
    fs::write(
        tmp.path().join("crates/core/Cargo.toml"),
        "[package]\nname = \"core\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    fs::write(tmp.path().join("crates/core/src/lib.rs"), "").unwrap();

    fs::create_dir_all(tmp.path().join("crates/cli/src")).unwrap();
    fs::write(
        tmp.path().join("crates/cli/Cargo.toml"),
        "[package]\nname = \"cli\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nclap = \"4\"\n",
    )
    .unwrap();
    fs::write(tmp.path().join("crates/cli/src/main.rs"), "fn main() {}").unwrap();

    let tree = build_project_tree(tmp.path()).unwrap();

    assert!(
        tree.len() >= 2,
        "should find at least 2 projects, got {}",
        tree.len()
    );
    assert_eq!(tree.workspace, Workspace::Cargo);
    assert_eq!(tree.topology, Topology::Workspace);
}

#[test]
fn pnpm_monorepo() {
    let tmp = make_repo();

    fs::write(tmp.path().join("package.json"), r#"{"name": "root"}"#).unwrap();
    fs::write(
        tmp.path().join("pnpm-workspace.yaml"),
        "packages:\n  - 'apps/web'\n  - 'apps/api'\n",
    )
    .unwrap();

    fs::create_dir_all(tmp.path().join("apps/web")).unwrap();
    fs::write(
        tmp.path().join("apps/web/package.json"),
        r#"{"name": "@my/web", "dependencies": {"react": "^18"}}"#,
    )
    .unwrap();

    fs::create_dir_all(tmp.path().join("apps/api")).unwrap();
    fs::write(
        tmp.path().join("apps/api/package.json"),
        r#"{"name": "@my/api", "dependencies": {"express": "^4"}}"#,
    )
    .unwrap();

    let tree = build_project_tree(tmp.path()).unwrap();

    assert!(
        tree.len() >= 3,
        "should find root + 2 apps, got {}",
        tree.len()
    );
    assert_eq!(tree.workspace, Workspace::Pnpm);
}

#[test]
fn empty_directory() {
    let tmp = make_repo();
    let tree = build_project_tree(tmp.path()).unwrap();
    assert!(tree.is_empty());
    assert_eq!(tree.topology, Topology::SingleProject);
}

#[test]
fn standalone_python_project() {
    let tmp = make_repo();
    fs::write(
        tmp.path().join("pyproject.toml"),
        "[project]\nname = \"myapp\"\nversion = \"0.1.0\"\ndependencies = [\"fastapi\"]\n",
    )
    .unwrap();

    let tree = build_project_tree(tmp.path()).unwrap();
    assert_eq!(tree.len(), 1);
    assert_eq!(tree.projects[0].language, crate::types::Language::Python);
}

#[test]
fn go_with_docker_compose() {
    let tmp = make_repo();

    // Two Go services with docker-compose
    fs::create_dir_all(tmp.path().join("auth")).unwrap();
    fs::write(tmp.path().join("auth/go.mod"), "module auth\ngo 1.21\n").unwrap();

    fs::create_dir_all(tmp.path().join("gateway")).unwrap();
    fs::write(
        tmp.path().join("gateway/go.mod"),
        "module gateway\ngo 1.21\n",
    )
    .unwrap();

    fs::write(
        tmp.path().join("docker-compose.yml"),
        "services:\n  auth:\n    build: ./auth\n  gateway:\n    build: ./gateway\n",
    )
    .unwrap();

    let tree = build_project_tree(tmp.path()).unwrap();

    // Should find at least the two Go services
    let go_projects: Vec<_> = tree
        .projects
        .iter()
        .filter(|p| p.language == crate::types::Language::Go)
        .collect();
    assert!(go_projects.len() >= 2, "should find 2 Go services");

    // Services referenced in docker-compose should be classified as Service
    for proj in &go_projects {
        assert_eq!(proj.role, ProjectRole::Service);
    }
}

#[test]
fn aspnet_with_nested_react() {
    let tmp = make_repo();

    // ASP.NET project with React SPA nested in ClientApp/
    fs::write(
        tmp.path().join("MyApp.csproj"),
        "<Project Sdk=\"Microsoft.NET.Sdk.Web\">\n  <PropertyGroup>\n    <TargetFramework>net8.0</TargetFramework>\n  </PropertyGroup>\n</Project>",
    )
    .unwrap();
    fs::write(tmp.path().join("Program.cs"), "").unwrap();
    fs::write(tmp.path().join("appsettings.json"), "{}").unwrap();

    fs::create_dir_all(tmp.path().join("ClientApp")).unwrap();
    fs::write(
        tmp.path().join("ClientApp/package.json"),
        r#"{"name": "client-app", "dependencies": {"react": "^18", "react-dom": "^18"}}"#,
    )
    .unwrap();

    let tree = build_project_tree(tmp.path()).unwrap();

    assert!(
        tree.len() >= 2,
        "should find ASP.NET + React, got {}",
        tree.len()
    );
    assert_eq!(tree.topology, Topology::NestedFullstack);

    // Verify hierarchy: ClientApp should be child of root
    let client = tree
        .projects
        .iter()
        .find(|p| p.path.to_string_lossy().contains("ClientApp"))
        .expect("should find ClientApp project");
    assert!(client.parent.is_some(), "ClientApp should have a parent");
    assert_eq!(client.role, ProjectRole::Frontend);
}

#[test]
fn nx_workspace() {
    let tmp = make_repo();

    // Nx workspace setup
    fs::write(tmp.path().join("nx.json"), r#"{"npmScope": "myorg"}"#).unwrap();
    fs::write(
        tmp.path().join("package.json"),
        r#"{"name": "root", "workspaces": ["apps/*", "libs/*"]}"#,
    )
    .unwrap();

    fs::create_dir_all(tmp.path().join("apps/web")).unwrap();
    fs::write(
        tmp.path().join("apps/web/package.json"),
        r#"{"name": "@myorg/web", "dependencies": {"react": "^18"}}"#,
    )
    .unwrap();

    fs::create_dir_all(tmp.path().join("apps/api")).unwrap();
    fs::write(
        tmp.path().join("apps/api/package.json"),
        r#"{"name": "@myorg/api", "dependencies": {"express": "^4"}}"#,
    )
    .unwrap();

    fs::create_dir_all(tmp.path().join("libs/shared")).unwrap();
    fs::write(
        tmp.path().join("libs/shared/package.json"),
        r#"{"name": "@myorg/shared"}"#,
    )
    .unwrap();

    let tree = build_project_tree(tmp.path()).unwrap();

    assert!(
        tree.len() >= 4,
        "should find root + 3 packages, got {}",
        tree.len()
    );
    assert_eq!(tree.workspace, Workspace::Nx);

    // Verify role classification
    let web = tree.projects.iter().find(|p| p.path.ends_with("apps/web"));
    assert!(web.is_some(), "should find web app");
    if let Some(web) = web {
        assert_eq!(web.role, ProjectRole::Frontend);
    }

    let api = tree.projects.iter().find(|p| p.path.ends_with("apps/api"));
    assert!(api.is_some(), "should find api app");
    if let Some(api) = api {
        assert_eq!(api.role, ProjectRole::Backend);
    }
}

#[test]
fn django_with_react_sibling() {
    let tmp = make_repo();

    // Python Django backend
    fs::create_dir_all(tmp.path().join("backend")).unwrap();
    fs::write(
        tmp.path().join("backend/pyproject.toml"),
        "[project]\nname = \"backend\"\nversion = \"0.1.0\"\ndependencies = [\"django\"]\n",
    )
    .unwrap();
    fs::write(tmp.path().join("backend/manage.py"), "").unwrap();

    // React frontend
    fs::create_dir_all(tmp.path().join("frontend")).unwrap();
    fs::write(
        tmp.path().join("frontend/package.json"),
        r#"{"name": "frontend", "dependencies": {"react": "^18"}}"#,
    )
    .unwrap();

    let tree = build_project_tree(tmp.path()).unwrap();

    assert!(tree.len() >= 2, "should find backend + frontend");

    let django = tree
        .projects
        .iter()
        .find(|p| p.language == crate::types::Language::Python);
    assert!(django.is_some(), "should find Django project");
    if let Some(d) = django {
        assert_eq!(d.role, ProjectRole::Backend);
    }

    let react = tree
        .projects
        .iter()
        .find(|p| p.path.to_string_lossy().contains("frontend"));
    assert!(react.is_some(), "should find React project");
    if let Some(r) = react {
        assert_eq!(r.role, ProjectRole::Frontend);
    }
}

#[test]
fn project_tree_json_roundtrip() {
    let tmp = make_repo();
    fs::write(
        tmp.path().join("Cargo.toml"),
        "[package]\nname = \"myapp\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();

    let tree = build_project_tree(tmp.path()).unwrap();
    let json = serde_json::to_string(&tree).unwrap();
    let deserialized: ProjectTree = serde_json::from_str(&json).unwrap();

    assert_eq!(tree.len(), deserialized.len());
    assert_eq!(tree.topology, deserialized.topology);
    assert_eq!(tree.workspace, deserialized.workspace);
}

#[test]
fn dual_manifest_root_splits_into_one_project_per_stack() {
    // The workbench-mcp shape: Cargo.toml AND package.json (+tsconfig, bun.lock)
    // at the same root. One stack must not shadow the other.
    let tmp = make_repo();
    fs::write(
        tmp.path().join("Cargo.toml"),
        "[package]\nname = \"myapp\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    fs::create_dir_all(tmp.path().join("src")).unwrap();
    fs::write(tmp.path().join("src/main.rs"), "fn main() {}").unwrap();
    fs::write(
        tmp.path().join("package.json"),
        "{\"name\": \"myapp\", \"devDependencies\": {\"typescript\": \"^5\"}}",
    )
    .unwrap();
    fs::write(tmp.path().join("tsconfig.json"), "{}").unwrap();

    let tree = build_project_tree(tmp.path()).unwrap();

    let root_langs: Vec<_> = tree
        .projects
        .iter()
        .filter(|p| p.path.as_os_str().is_empty())
        .map(|p| p.language)
        .collect();
    assert_eq!(
        root_langs,
        vec![
            crate::types::Language::Rust,
            crate::types::Language::TypeScript
        ],
        "the root dir carries two stacks and must yield two projects, primary first"
    );
}

#[test]
fn single_manifest_dirs_are_not_split() {
    let tmp = make_repo();
    fs::write(
        tmp.path().join("Cargo.toml"),
        "[package]\nname = \"myapp\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();

    let tree = build_project_tree(tmp.path()).unwrap();
    assert_eq!(tree.len(), 1);
}
