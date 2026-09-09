use super::*;
use tempfile::TempDir;

fn touch(dir: &Path, name: &str) {
    std::fs::write(dir.join(name), "").unwrap();
}

fn touch_with(dir: &Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).unwrap();
}

#[test]
fn detects_cargo_workspace() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/*\"]\n",
    );
    assert_eq!(detect_workspace(tmp.path()), Workspace::Cargo);
}

#[test]
fn ignores_cargo_without_workspace() {
    let tmp = TempDir::new().unwrap();
    touch_with(tmp.path(), "Cargo.toml", "[package]\nname = \"single\"\n");
    assert_eq!(detect_workspace(tmp.path()), Workspace::None);
}

#[test]
fn detects_turborepo() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "turbo.json");
    touch_with(
        tmp.path(),
        "package.json",
        r#"{"workspaces":["packages/*"]}"#,
    );
    assert_eq!(detect_workspace(tmp.path()), Workspace::Turborepo);
}

#[test]
fn detects_nx() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "nx.json");
    assert_eq!(detect_workspace(tmp.path()), Workspace::Nx);
}

#[test]
fn detects_pnpm_workspace() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "pnpm-workspace.yaml",
        "packages:\n  - 'packages/*'\n",
    );
    assert_eq!(detect_workspace(tmp.path()), Workspace::Pnpm);
}

#[test]
fn detects_npm_workspaces() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "package.json",
        r#"{"name":"root","workspaces":["packages/*"]}"#,
    );
    assert_eq!(detect_workspace(tmp.path()), Workspace::Npm);
}

#[test]
fn detects_yarn_workspaces() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "package.json",
        r#"{"name":"root","workspaces":["packages/*"]}"#,
    );
    touch(tmp.path(), "yarn.lock");
    assert_eq!(detect_workspace(tmp.path()), Workspace::Yarn);
}

#[test]
fn no_workspace_for_plain_project() {
    let tmp = TempDir::new().unwrap();
    touch_with(tmp.path(), "package.json", r#"{"name":"app"}"#);
    assert_eq!(detect_workspace(tmp.path()), Workspace::None);
}

#[test]
fn no_workspace_for_empty_dir() {
    let tmp = TempDir::new().unwrap();
    assert_eq!(detect_workspace(tmp.path()), Workspace::None);
}
