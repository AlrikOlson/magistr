use super::*;
use crate::types::*;
use tempfile::TempDir;

#[test]
fn detects_full_rust_stack() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(
        tmp.path().join("Cargo.toml"),
        "[package]\nname = \"my-project\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();

    let stack = detect_stack(tmp.path()).unwrap();
    assert_eq!(stack.language, Language::Rust);
    assert_eq!(stack.package_manager, PackageManager::Cargo);
    assert_eq!(stack.framework, Framework::None);
    assert_eq!(stack.test_framework, TestFramework::CargoTest);
    assert_eq!(stack.linter, Linter::Clippy);
    assert_eq!(stack.formatter, Formatter::Rustfmt);
    assert_eq!(stack.project_name, "my-project");
}

#[test]
fn detects_full_ts_next_stack() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(
            tmp.path().join("package.json"),
            r#"{"name":"my-app","dependencies":{"next":"14","react":"18"},"devDependencies":{"typescript":"5"}}"#,
        )
        .unwrap();
    std::fs::write(tmp.path().join("tsconfig.json"), "{}").unwrap();
    std::fs::write(tmp.path().join("next.config.js"), "").unwrap();
    std::fs::write(tmp.path().join("vitest.config.ts"), "").unwrap();

    let stack = detect_stack(tmp.path()).unwrap();
    assert_eq!(stack.language, Language::TypeScript);
    assert_eq!(stack.package_manager, PackageManager::Npm);
    assert_eq!(stack.framework, Framework::Next);
    assert_eq!(stack.test_framework, TestFramework::Vitest);
    assert_eq!(stack.linter, Linter::Eslint);
    assert_eq!(stack.formatter, Formatter::Prettier);
    assert_eq!(stack.project_name, "my-app");
}

#[test]
fn detects_full_python_fastapi_stack() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(
            tmp.path().join("pyproject.toml"),
            "[project]\nname = \"my-api\"\ndependencies = [\"fastapi\"]\n\n[tool.ruff]\nline-length = 100\n",
        )
        .unwrap();

    let stack = detect_stack(tmp.path()).unwrap();
    assert_eq!(stack.language, Language::Python);
    assert_eq!(stack.framework, Framework::FastApi);
    assert_eq!(stack.linter, Linter::Ruff);
    assert_eq!(stack.formatter, Formatter::Ruff);
    assert_eq!(stack.project_name, "my-api");
}

#[test]
fn detects_rust_axum_with_workspace() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/*\"]\n\n[package]\nname = \"my-api\"\n\n[dependencies]\naxum = \"0.7\"\ntokio = { version = \"1\", features = [\"full\"] }\n",
        )
        .unwrap();

    let stack = detect_stack(tmp.path()).unwrap();
    assert_eq!(stack.language, Language::Rust);
    assert_eq!(stack.framework, Framework::Axum);
    assert_eq!(stack.workspace, Workspace::Cargo);
}

#[test]
fn detects_turborepo_next_stack() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(
            tmp.path().join("package.json"),
            r#"{"name":"monorepo","workspaces":["apps/*","packages/*"],"devDependencies":{"typescript":"5"}}"#,
        )
        .unwrap();
    std::fs::write(tmp.path().join("turbo.json"), "{}").unwrap();
    std::fs::write(tmp.path().join("tsconfig.json"), "{}").unwrap();
    std::fs::write(tmp.path().join("next.config.js"), "").unwrap();

    let stack = detect_stack(tmp.path()).unwrap();
    assert_eq!(stack.language, Language::TypeScript);
    assert_eq!(stack.framework, Framework::Next);
    assert_eq!(stack.workspace, Workspace::Turborepo);
}

#[test]
fn errors_on_nonexistent_dir() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let result = detect_stack(&tmp.path().join("nonexistent"));
    assert!(result.is_err());
}

#[test]
fn detect_stack_has_empty_extra_detections_by_default() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(
        tmp.path().join("Cargo.toml"),
        "[package]\nname = \"test\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();

    let stack = detect_stack(tmp.path()).unwrap();
    assert!(stack.extra_detections.is_empty());
}

#[test]
fn detect_stack_with_plugins_merges_extras() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(
        tmp.path().join("Cargo.toml"),
        "[package]\nname = \"test\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();

    let extras = vec![PluginDetection {
        category: "tool".into(),
        name: "custom-tool".into(),
        confidence: 90,
        plugin_name: "test-plugin".into(),
    }];

    let stack = detect_stack_with_plugins(tmp.path(), extras).unwrap();
    assert_eq!(stack.extra_detections.len(), 1);
    assert_eq!(stack.extra_detections[0].name, "custom-tool");
}
