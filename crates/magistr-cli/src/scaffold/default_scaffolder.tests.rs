use super::*;

use std::collections::HashMap;
use std::io::Write;
use tempfile::TempDir;

use crate::scaffold::scaffolder::Scaffolder;

#[test]
fn default_scaffolder_implements_trait() {
    // Verify DefaultScaffolder can be used as dyn Scaffolder
    let _scaffolder: Box<dyn Scaffolder> = Box::new(DefaultScaffolder);
}

#[test]
fn default_scaffolder_generates_files() {
    let tmp = TempDir::new().unwrap();

    // Create a minimal Cargo.toml so detection works
    let cargo_toml = tmp.path().join("Cargo.toml");
    let mut f = std::fs::File::create(&cargo_toml).unwrap();
    writeln!(
        f,
        r#"[package]
name = "test-crate"
version = "0.1.0"
edition = "2021""#
    )
    .unwrap();

    let mut vars = HashMap::new();
    vars.insert("language".to_string(), "rust".to_string());
    vars.insert("framework".to_string(), "none".to_string());
    vars.insert("project_name".to_string(), "test-crate".to_string());
    vars.insert("check_command".to_string(), "cargo check".to_string());
    vars.insert("test_command".to_string(), "cargo test".to_string());
    vars.insert("lint_command".to_string(), "cargo clippy".to_string());
    vars.insert("format_command".to_string(), "cargo fmt".to_string());
    vars.insert("package_manager".to_string(), "cargo".to_string());
    vars.insert("test_framework".to_string(), "built_in".to_string());
    vars.insert("linter".to_string(), "clippy".to_string());
    vars.insert("formatter".to_string(), "rustfmt".to_string());

    let scaffolder = DefaultScaffolder;
    let result = scaffolder
        .generate(tmp.path(), &vars, &HashMap::new())
        .unwrap();

    assert!(!result.is_empty(), "should generate at least some files");
}
