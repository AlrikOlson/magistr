use super::*;
use std::fs;

#[test]
fn detect_cargo_path_deps_finds_edges() {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path().canonicalize().unwrap();

    // Create two crates with a path dependency
    fs::create_dir_all(root.join("crates/core/src")).unwrap();
    fs::create_dir_all(root.join("crates/cli/src")).unwrap();
    fs::write(
        root.join("crates/core/Cargo.toml"),
        "[package]\nname = \"core\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        root.join("crates/cli/Cargo.toml"),
        "[package]\nname = \"cli\"\nversion = \"0.1.0\"\n\n[dependencies]\ncore = { path = \"../core\" }\n",
    )
    .unwrap();

    let path_index: HashMap<&Path, usize> = [
        (Path::new("crates/core"), 0usize),
        (Path::new("crates/cli"), 1usize),
    ]
    .into_iter()
    .collect();

    let edges = detect_relationships(&root, &path_index);
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].from, 1); // cli depends on
    assert_eq!(edges[0].to, 0); // core
    assert_eq!(edges[0].kind, EdgeKind::CargoPathDep);
}

#[test]
fn detect_npm_workspace_refs_finds_edges() {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path().canonicalize().unwrap();

    fs::create_dir_all(root.join("packages/shared")).unwrap();
    fs::create_dir_all(root.join("apps/web")).unwrap();

    fs::write(
        root.join("packages/shared/package.json"),
        r#"{"name": "@my/shared", "version": "1.0.0"}"#,
    )
    .unwrap();
    fs::write(
        root.join("apps/web/package.json"),
        r#"{"name": "@my/web", "dependencies": {"@my/shared": "workspace:*"}}"#,
    )
    .unwrap();

    let path_index: HashMap<&Path, usize> = [
        (Path::new("packages/shared"), 0usize),
        (Path::new("apps/web"), 1usize),
    ]
    .into_iter()
    .collect();

    let edges = detect_relationships(&root, &path_index);
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].from, 1); // web depends on
    assert_eq!(edges[0].to, 0); // shared
    assert_eq!(edges[0].kind, EdgeKind::NpmWorkspaceRef);
}

#[test]
fn detect_csproj_refs_finds_edges() {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path().canonicalize().unwrap();

    fs::create_dir_all(root.join("src/Core")).unwrap();
    fs::create_dir_all(root.join("src/Web")).unwrap();

    fs::write(root.join("src/Core/Core.csproj"), "<Project />").unwrap();
    fs::write(
        root.join("src/Web/Web.csproj"),
        r#"<Project>
  <ItemGroup>
    <ProjectReference Include="..\Core\Core.csproj" />
  </ItemGroup>
</Project>"#,
    )
    .unwrap();

    let path_index: HashMap<&Path, usize> = [
        (Path::new("src/Core"), 0usize),
        (Path::new("src/Web"), 1usize),
    ]
    .into_iter()
    .collect();

    let edges = detect_relationships(&root, &path_index);
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].from, 1); // Web depends on
    assert_eq!(edges[0].to, 0); // Core
    assert_eq!(edges[0].kind, EdgeKind::CsprojRef);
}

#[test]
fn no_self_edges() {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path().canonicalize().unwrap();

    fs::write(root.join("package.json"), r#"{"name": "app"}"#).unwrap();

    let path_index: HashMap<&Path, usize> = [(Path::new(""), 0usize)].into_iter().collect();

    let edges = detect_relationships(&root, &path_index);
    assert!(edges.is_empty(), "should not create self-edges");
}
