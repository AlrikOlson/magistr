use super::*;
use crate::detection::detect::config_parser::parse_manifests;
use tempfile::TempDir;

fn touch(dir: &Path, name: &str) {
    std::fs::write(dir.join(name), "").unwrap();
}

fn touch_with(dir: &Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).unwrap();
}

#[test]
fn javascript_reports_only_observed_tools() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "package.json",
        r#"{"scripts":{"release":"bun build src/index.ts"}}"#,
    );
    let m = parse_manifests(tmp.path());
    assert_eq!(
        detect_test_framework(tmp.path(), Language::TypeScript, &m),
        TestFramework::Unknown
    );
    assert_eq!(
        detect_linter(tmp.path(), Language::TypeScript, &m),
        Linter::None
    );
    assert_eq!(
        detect_formatter(tmp.path(), Language::TypeScript, &m),
        Formatter::None
    );
    touch_with(
        tmp.path(),
        "package.json",
        r#"{"devDependencies":{"eslint":"9","prettier":"3","vitest":"3"}}"#,
    );
    let m = parse_manifests(tmp.path());
    assert_eq!(
        detect_test_framework(tmp.path(), Language::TypeScript, &m),
        TestFramework::Vitest
    );
    assert_eq!(
        detect_linter(tmp.path(), Language::TypeScript, &m),
        Linter::Eslint
    );
    assert_eq!(
        detect_formatter(tmp.path(), Language::TypeScript, &m),
        Formatter::Prettier
    );
}

// Test framework tests
#[test]
fn rust_uses_cargo_test() {
    let tmp = TempDir::new().unwrap();
    let m = parse_manifests(tmp.path());
    assert_eq!(
        detect_test_framework(tmp.path(), Language::Rust, &m),
        TestFramework::CargoTest
    );
}

#[test]
fn js_detects_vitest() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "vitest.config.ts");
    let m = parse_manifests(tmp.path());
    assert_eq!(
        detect_test_framework(tmp.path(), Language::TypeScript, &m),
        TestFramework::Vitest
    );
}

#[test]
fn js_detects_jest() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "jest.config.ts");
    let m = parse_manifests(tmp.path());
    assert_eq!(
        detect_test_framework(tmp.path(), Language::TypeScript, &m),
        TestFramework::Jest
    );
}

#[test]
fn js_detects_jest_from_package() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "package.json",
        r#"{"devDependencies":{"jest":"^29"}}"#,
    );
    let m = parse_manifests(tmp.path());
    assert_eq!(
        detect_test_framework(tmp.path(), Language::JavaScript, &m),
        TestFramework::Jest
    );
}

#[test]
fn ruby_detects_rspec() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), ".rspec");
    let m = parse_manifests(tmp.path());
    assert_eq!(
        detect_test_framework(tmp.path(), Language::Ruby, &m),
        TestFramework::Rspec
    );
}

#[test]
fn ruby_detects_rspec_from_gemfile() {
    let tmp = TempDir::new().unwrap();
    touch_with(tmp.path(), "Gemfile", "gem 'rspec'\n");
    let m = parse_manifests(tmp.path());
    assert_eq!(
        detect_test_framework(tmp.path(), Language::Ruby, &m),
        TestFramework::Rspec
    );
}

// Linter tests
#[test]
fn js_detects_biome_linter() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "biome.json");
    let m = parse_manifests(tmp.path());
    assert_eq!(
        detect_linter(tmp.path(), Language::TypeScript, &m),
        Linter::Biome
    );
}

#[test]
fn js_detects_biome_from_dep() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "package.json",
        r#"{"devDependencies":{"@biomejs/biome":"^1"}}"#,
    );
    let m = parse_manifests(tmp.path());
    assert_eq!(
        detect_linter(tmp.path(), Language::TypeScript, &m),
        Linter::Biome
    );
}

#[test]
fn python_detects_ruff() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "ruff.toml");
    let m = parse_manifests(tmp.path());
    assert_eq!(
        detect_linter(tmp.path(), Language::Python, &m),
        Linter::Ruff
    );
}

#[test]
fn python_detects_ruff_from_pyproject() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "pyproject.toml",
        "[project]\nname = \"app\"\n\n[tool.ruff]\nline-length = 88\n",
    );
    let m = parse_manifests(tmp.path());
    assert_eq!(
        detect_linter(tmp.path(), Language::Python, &m),
        Linter::Ruff
    );
}

// Formatter tests
#[test]
fn js_detects_biome_formatter() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "biome.json");
    let m = parse_manifests(tmp.path());
    assert_eq!(
        detect_formatter(tmp.path(), Language::TypeScript, &m),
        Formatter::Biome
    );
}

#[test]
fn python_without_formatter_evidence_is_unconfigured() {
    let tmp = TempDir::new().unwrap();
    let m = parse_manifests(tmp.path());
    assert_eq!(
        detect_formatter(tmp.path(), Language::Python, &m),
        Formatter::None
    );
}

#[test]
fn python_detects_ruff_formatter_from_pyproject() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "pyproject.toml",
        "[project]\nname = \"app\"\n\n[tool.ruff]\nline-length = 88\n",
    );
    let m = parse_manifests(tmp.path());
    assert_eq!(
        detect_formatter(tmp.path(), Language::Python, &m),
        Formatter::Ruff
    );
}
