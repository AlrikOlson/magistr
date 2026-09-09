use super::*;
use std::fs;

#[test]
fn infer_hierarchy_flat_projects() {
    let paths = vec![
        PathBuf::from(""),
        PathBuf::from("apps/web"),
        PathBuf::from("apps/api"),
    ];
    let (parents, children) = infer_hierarchy(&paths);

    // Root has no parent
    assert_eq!(parents[0], None);
    // apps/web and apps/api are children of root
    assert_eq!(parents[1], Some(0));
    assert_eq!(parents[2], Some(0));
    // Root has 2 children
    assert_eq!(children[&0].len(), 2);
}

#[test]
fn infer_hierarchy_nested_project() {
    let paths = vec![
        PathBuf::from(""),
        PathBuf::from("src/backend"),
        PathBuf::from("src/backend/ClientApp"),
    ];
    let (parents, _children) = infer_hierarchy(&paths);

    assert_eq!(parents[0], None);
    assert_eq!(parents[1], Some(0)); // src/backend → root
    assert_eq!(parents[2], Some(1)); // ClientApp → src/backend
}

#[test]
fn classify_topology_single() {
    assert_eq!(
        classify_topology(1, Workspace::None, false),
        Topology::SingleProject
    );
}

#[test]
fn classify_topology_workspace() {
    assert_eq!(
        classify_topology(3, Workspace::Cargo, false),
        Topology::Workspace
    );
}

#[test]
fn classify_topology_nested() {
    assert_eq!(
        classify_topology(2, Workspace::None, true),
        Topology::NestedFullstack
    );
}

#[test]
fn classify_topology_flat_monorepo() {
    assert_eq!(
        classify_topology(3, Workspace::None, false),
        Topology::FlatMonorepo
    );
}

#[test]
fn parse_cargo_workspace_members_basic() {
    let tmp = tempfile::TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join("crates/core")).unwrap();
    fs::create_dir_all(tmp.path().join("crates/cli")).unwrap();
    fs::write(
        tmp.path().join("Cargo.toml"),
        r#"
[workspace]
members = ["crates/core", "crates/cli"]
"#,
    )
    .unwrap();

    let members = parse_cargo_workspace_members(tmp.path());
    assert_eq!(members.len(), 2);
    assert!(members.contains(&PathBuf::from("crates/core")));
    assert!(members.contains(&PathBuf::from("crates/cli")));
}

#[test]
fn parse_pnpm_workspace_basic() {
    let tmp = tempfile::TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join("apps/web")).unwrap();
    fs::create_dir_all(tmp.path().join("apps/api")).unwrap();
    fs::write(
        tmp.path().join("pnpm-workspace.yaml"),
        "packages:\n  - 'apps/web'\n  - 'apps/api'\n",
    )
    .unwrap();

    let packages = parse_pnpm_workspace_packages(tmp.path());
    assert_eq!(packages.len(), 2);
}

#[test]
fn parse_npm_workspaces_array() {
    let tmp = tempfile::TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join("packages/shared")).unwrap();
    fs::write(
        tmp.path().join("package.json"),
        r#"{"workspaces": ["packages/shared"]}"#,
    )
    .unwrap();

    let packages = parse_npm_workspaces(tmp.path());
    assert_eq!(packages.len(), 1);
    assert_eq!(packages[0], PathBuf::from("packages/shared"));
}

#[test]
fn parse_lerna_packages_basic() {
    let tmp = tempfile::TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join("packages/ui")).unwrap();
    fs::write(
        tmp.path().join("lerna.json"),
        r#"{"packages": ["packages/ui"]}"#,
    )
    .unwrap();

    let packages = parse_lerna_packages(tmp.path());
    assert_eq!(packages.len(), 1);
}

#[test]
fn parse_docker_compose_services_basic() {
    let tmp = tempfile::TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join("api")).unwrap();
    fs::create_dir_all(tmp.path().join("web")).unwrap();
    fs::write(
        tmp.path().join("docker-compose.yml"),
        "services:\n  api:\n    build: ./api\n  web:\n    build: ./web\n",
    )
    .unwrap();

    let services = parse_docker_compose_services(tmp.path());
    assert_eq!(services.len(), 2);
    assert!(services.contains(&PathBuf::from("api")));
    assert!(services.contains(&PathBuf::from("web")));
}

#[test]
fn parse_sln_projects_basic() {
    let tmp = tempfile::TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join("src/MyApp")).unwrap();
    fs::write(
        tmp.path().join("MySolution.sln"),
        r#"
Project("{FAE04EC0-301F-11D3-BF4B-00C04F79EFBC}") = "MyApp", "src\MyApp\MyApp.csproj", "{GUID}"
EndProject
"#,
    )
    .unwrap();

    let projects = parse_sln_projects(tmp.path());
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0], PathBuf::from("src/MyApp"));
}
