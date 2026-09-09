use super::*;
use tempfile::TempDir;

#[test]
fn extracts_from_cargo_toml() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(
        tmp.path().join("Cargo.toml"),
        "[package]\nname = \"my-crate\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    assert_eq!(detect_project_name(tmp.path(), Language::Rust), "my-crate");
}

#[test]
fn extracts_from_package_json() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(
        tmp.path().join("package.json"),
        r#"{"name":"my-app","version":"1.0.0"}"#,
    )
    .unwrap();
    assert_eq!(
        detect_project_name(tmp.path(), Language::TypeScript),
        "my-app"
    );
}

#[test]
fn extracts_from_go_mod() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(
        tmp.path().join("go.mod"),
        "module github.com/user/myservice\n\ngo 1.21\n",
    )
    .unwrap();
    assert_eq!(detect_project_name(tmp.path(), Language::Go), "myservice");
}

#[test]
fn falls_back_to_dir_name() {
    let tmp = TempDir::new().unwrap();
    let name = detect_project_name(tmp.path(), Language::Unknown);
    // tempdir names are random, just verify it's not empty
    assert!(!name.is_empty());
}
