use super::*;
use tempfile::TempDir;

fn write_file(dir: &Path, name: &str, content: &str) {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, content).unwrap();
}

fn valid_manifest() -> &'static str {
    r#"
name = "test-plugin"
version = "1.0.0"
kind = "detection"
description = "A test detection plugin"
"#
}

fn valid_template_manifest() -> &'static str {
    r#"
name = "test-templates"
version = "1.0.0"
kind = "template"
description = "A test template plugin"
"#
}

// ── load_manifest tests ────────────────────────────────────

#[test]
fn load_manifest_success() {
    let dir = TempDir::new().unwrap();
    write_file(dir.path(), MANIFEST_FILENAME, valid_manifest());

    let manifest = load_manifest(dir.path()).unwrap();
    assert_eq!(manifest.name, "test-plugin");
    assert_eq!(manifest.kind, PluginKind::Detection);
}

#[test]
fn load_manifest_dir_not_found() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let result = load_manifest(&tmp.path().join("nonexistent"));
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("not found"), "{err}");
}

#[test]
fn load_manifest_file_missing() {
    let dir = TempDir::new().unwrap();
    let result = load_manifest(dir.path());
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains(MANIFEST_FILENAME));
}

#[test]
fn load_manifest_invalid_toml() {
    let dir = TempDir::new().unwrap();
    write_file(dir.path(), MANIFEST_FILENAME, "invalid {{{ toml");

    let result = load_manifest(dir.path());
    assert!(result.is_err());
}

// ── load_detection_config tests ────────────────────────────

#[test]
fn load_detection_config_success() {
    let dir = TempDir::new().unwrap();
    write_file(
        dir.path(),
        "detect.toml",
        r#"
[[rules]]
name = "my-framework"
file_patterns = ["framework.config.js"]
confidence = 90
"#,
    );

    let config = load_detection_config(dir.path()).unwrap();
    assert_eq!(config.rules.len(), 1);
    assert_eq!(config.rules[0].name, "my-framework");
}

#[test]
fn load_detection_config_missing_returns_empty() {
    let dir = TempDir::new().unwrap();
    let config = load_detection_config(dir.path()).unwrap();
    assert!(config.rules.is_empty());
}

// ── load_template_config tests ─────────────────────────────

#[test]
fn load_template_config_success() {
    let dir = TempDir::new().unwrap();
    write_file(
        dir.path(),
        "templates.toml",
        r#"
[[files]]
src = "rules/custom.md"
dest = "rules/custom.md"
"#,
    );

    let config = load_template_config(dir.path()).unwrap();
    assert_eq!(config.files.len(), 1);
    assert_eq!(config.files[0].src, "rules/custom.md");
}

#[test]
fn load_template_config_missing_returns_empty() {
    let dir = TempDir::new().unwrap();
    let config = load_template_config(dir.path()).unwrap();
    assert!(config.files.is_empty());
}

// ── validate_plugin tests ──────────────────────────────────

#[test]
fn validate_detection_plugin_complete() {
    let dir = TempDir::new().unwrap();
    write_file(dir.path(), MANIFEST_FILENAME, valid_manifest());
    write_file(
        dir.path(),
        "detect.toml",
        r#"
[[rules]]
name = "my-rule"
file_patterns = ["config.yaml"]
"#,
    );

    let warnings = validate_plugin(dir.path()).unwrap();
    assert!(warnings.is_empty());
}

#[test]
fn validate_detection_plugin_missing_detect_toml() {
    let dir = TempDir::new().unwrap();
    write_file(dir.path(), MANIFEST_FILENAME, valid_manifest());

    let warnings = validate_plugin(dir.path()).unwrap();
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("missing detect.toml"));
}

#[test]
fn validate_detection_plugin_empty_rules() {
    let dir = TempDir::new().unwrap();
    write_file(dir.path(), MANIFEST_FILENAME, valid_manifest());
    write_file(dir.path(), "detect.toml", "");

    let warnings = validate_plugin(dir.path()).unwrap();
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("no rules"));
}

#[test]
fn validate_template_plugin_complete() {
    let dir = TempDir::new().unwrap();
    write_file(dir.path(), MANIFEST_FILENAME, valid_template_manifest());
    write_file(
        dir.path(),
        "templates.toml",
        r#"
[[files]]
src = "rules/custom.md"
dest = "rules/custom.md"
"#,
    );
    write_file(dir.path(), "rules/custom.md", "# Custom Rules\n");

    let warnings = validate_plugin(dir.path()).unwrap();
    assert!(warnings.is_empty());
}

#[test]
fn validate_template_plugin_missing_source_file() {
    let dir = TempDir::new().unwrap();
    write_file(dir.path(), MANIFEST_FILENAME, valid_template_manifest());
    write_file(
        dir.path(),
        "templates.toml",
        r#"
[[files]]
src = "rules/missing.md"
dest = "rules/missing.md"
"#,
    );

    let warnings = validate_plugin(dir.path()).unwrap();
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("not found"));
}

#[test]
fn validate_no_manifest_errors() {
    let dir = TempDir::new().unwrap();
    let result = validate_plugin(dir.path());
    assert!(result.is_err());
}

// ── Executable plugin validation tests ─────────────────────

fn valid_step_manifest() -> &'static str {
    r#"
name = "test-step"
version = "1.0.0"
kind = "workflow-step"
entrypoint = "bin/step"
permissions = ["network", "env-read"]
"#
}

#[test]
fn validate_step_plugin_with_entrypoint() {
    let dir = TempDir::new().unwrap();
    write_file(dir.path(), MANIFEST_FILENAME, valid_step_manifest());
    write_file(dir.path(), "bin/step", "#!/bin/sh\necho ok");

    let warnings = validate_plugin(dir.path()).unwrap();
    assert!(warnings.is_empty());
}

#[test]
fn validate_step_plugin_missing_entrypoint_file() {
    let dir = TempDir::new().unwrap();
    write_file(dir.path(), MANIFEST_FILENAME, valid_step_manifest());
    // Don't create the entrypoint file

    let warnings = validate_plugin(dir.path()).unwrap();
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("entrypoint"));
    assert!(warnings[0].contains("not found"));
}

#[test]
fn validate_step_plugin_no_entrypoint_field_errors() {
    let dir = TempDir::new().unwrap();
    write_file(
        dir.path(),
        MANIFEST_FILENAME,
        r#"
name = "bad-step"
version = "1.0.0"
kind = "workflow-step"
"#,
    );

    let result = validate_plugin(dir.path());
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("requires an entrypoint"));
}

/// A plugin directory from the bridge era — `kind = "backend"` — is
/// refused at load, and the refusal names the way a model is reached now.
#[test]
fn load_manifest_refuses_a_backend_plugin_naming_the_acp_agent() {
    let dir = TempDir::new().unwrap();
    write_file(
        dir.path(),
        MANIFEST_FILENAME,
        r#"
name = "openai-backend"
version = "1.0.0"
kind = "backend"
entrypoint = "bin/openai"
"#,
    );
    write_file(dir.path(), "bin/openai", "#!/bin/sh\necho ok");

    let err = load_manifest(dir.path()).unwrap_err().to_string();
    assert!(err.contains("magistr acp-agent"), "{err}");
    let err = validate_plugin(dir.path()).unwrap_err().to_string();
    assert!(err.contains("magistr acp-agent"), "{err}");
}

#[test]
fn validate_quality_gate_plugin() {
    let dir = TempDir::new().unwrap();
    write_file(
        dir.path(),
        MANIFEST_FILENAME,
        r#"
name = "sonar-gate"
version = "0.5.0"
kind = "quality-gate"
entrypoint = "bin/sonar"
permissions = ["network", "file-read"]
"#,
    );
    write_file(dir.path(), "bin/sonar", "#!/bin/sh\necho ok");

    let warnings = validate_plugin(dir.path()).unwrap();
    assert!(warnings.is_empty());
}

#[test]
fn validate_workflow_step_plugin() {
    let dir = TempDir::new().unwrap();
    write_file(
        dir.path(),
        MANIFEST_FILENAME,
        r#"
name = "pre-review"
version = "1.0.0"
kind = "workflow-step"
entrypoint = "bin/pre-review"
"#,
    );
    write_file(dir.path(), "bin/pre-review", "#!/bin/sh\necho ok");

    let warnings = validate_plugin(dir.path()).unwrap();
    assert!(warnings.is_empty());
}

#[test]
fn load_manifest_backend_with_dependencies() {
    let dir = TempDir::new().unwrap();
    write_file(
        dir.path(),
        MANIFEST_FILENAME,
        r#"
name = "sonar-gate"
version = "1.0.0"
kind = "quality-gate"
entrypoint = "bin/sonar"
permissions = ["network", "env-read"]

[[dependencies]]
name = "auth-helper"
version = ">=1.0.0"

[compatibility]
jig_version = "0.2.0"
"#,
    );

    let manifest = load_manifest(dir.path()).unwrap();
    assert_eq!(manifest.kind, PluginKind::QualityGate);
    assert_eq!(manifest.entrypoint.as_deref(), Some("bin/sonar"));
    assert_eq!(manifest.dependencies.len(), 1);
    assert_eq!(manifest.dependencies[0].name, "auth-helper");
    assert_eq!(manifest.compatibility.jig_version.as_deref(), Some("0.2.0"));
}
