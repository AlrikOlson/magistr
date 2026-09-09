use super::*;

// ── TempProject tests ───────────────────────────────────────────────────────

#[test]
fn temp_project_creates_directory() {
    let project = TempProject::new();
    assert!(project.path().exists());
    assert!(project.path().is_dir());
}

#[test]
fn temp_project_path_str_is_valid() {
    let project = TempProject::new();
    let s = project.path_str();
    assert!(!s.is_empty());
    assert!(Path::new(s).exists());
}

#[test]
fn temp_project_with_magistr_dir() {
    let project = TempProject::with_magistr_dir();
    assert!(project.path().join(".magistr").exists());
}

#[test]
fn temp_project_write_and_read_file() {
    let project = TempProject::new();
    project.write_file("src/main.rs", "fn main() {}");
    assert!(project.file_exists("src/main.rs"));
    assert_eq!(project.read_file("src/main.rs"), "fn main() {}");
}

#[test]
fn temp_project_file_exists_returns_false_for_missing() {
    let project = TempProject::new();
    assert!(!project.file_exists("nonexistent.rs"));
}

// ── MockProjectDir builder tests ────────────────────────────────────────────

#[test]
fn builder_bare_creates_empty_dir() {
    let project = MockProjectDir::new().build();
    assert!(project.path().exists());
    assert!(!project.file_exists(".magistr"));
}

#[test]
fn builder_with_magistr_dir() {
    let project = MockProjectDir::new().with_magistr_dir().build();
    assert!(project.file_exists(".magistr"));
}

#[test]
fn builder_with_config() {
    let project = MockProjectDir::new()
        .with_config(serde_json::json!({"language": "rust"}))
        .build();
    assert!(project.file_exists(".magistr/project.json"));
    let content = project.read_file(".magistr/project.json");
    assert!(content.contains("rust"));
}

#[test]
fn builder_with_files() {
    let project = MockProjectDir::new()
        .with_file("src/lib.rs", b"pub mod foo;".to_vec())
        .with_file("Cargo.toml", b"[package]".to_vec())
        .build();
    assert!(project.file_exists("src/lib.rs"));
    assert!(project.file_exists("Cargo.toml"));
}

// ── MockGatePlugin tests ────────────────────────────────────────────────────

#[tokio::test]
async fn mock_gate_passing() {
    let mut gate = MockGatePlugin::passing("test-gate");
    let config = GateConfig {
        gate_name: "test-gate".to_string(),
        project_dir: "/unused".to_string(),
        config: None,
    };
    let outcome = gate.run_gate(config).await.unwrap();
    assert!(outcome.passed);
    assert_eq!(gate.calls.len(), 1);
    assert_eq!(gate.calls[0].0, "test-gate");
}

#[tokio::test]
async fn mock_gate_failing() {
    use crate::plugin::gate_trait::GateSeverity;

    let finding = GateFinding {
        file: Some("src/main.rs".to_string()),
        line: Some(10),
        message: "unused variable".to_string(),
        severity: GateSeverity::Error,
        rule_id: Some("E001".to_string()),
    };
    let mut gate = MockGatePlugin::failing("lint", vec![finding]);
    let config = GateConfig {
        gate_name: "lint".to_string(),
        project_dir: "/unused".to_string(),
        config: None,
    };
    let outcome = gate.run_gate(config).await.unwrap();
    assert!(!outcome.passed);
    assert_eq!(outcome.error_count(), 1);
}

#[test]
fn mock_gate_name_and_supported_gates() {
    let gate = MockGatePlugin::passing("sonar")
        .with_gates(vec!["sonarqube".to_string(), "security".to_string()]);
    assert_eq!(gate.name(), "sonar");
    assert_eq!(gate.supported_gates().len(), 2);
}

// ── MockOutputRenderer tests ────────────────────────────────────────────────

#[test]
fn mock_output_renderer_captures_text() {
    let renderer = MockOutputRenderer::new();
    let mut buf = Vec::new();
    renderer.success(&mut buf, "done").unwrap();
    renderer.error(&mut buf, "oops").unwrap();
    renderer.warning(&mut buf, "careful").unwrap();
    renderer.info(&mut buf, "fyi").unwrap();

    let lines = renderer.lines();
    assert_eq!(lines.len(), 4);
    assert!(renderer.contains("done"));
    assert!(renderer.contains("oops"));
}

#[test]
fn mock_output_renderer_captures_data() {
    let renderer = MockOutputRenderer::new();
    let mut buf = Vec::new();
    renderer
        .table(&mut buf, &["a".into()], &[vec!["1".into()]])
        .unwrap();
    renderer
        .json(&mut buf, &serde_json::json!({"k": "v"}))
        .unwrap();
    renderer.kv(&mut buf, &[("a".into(), "b".into())]).unwrap();
    renderer.list(&mut buf, &["x".into(), "y".into()]).unwrap();

    let lines = renderer.lines();
    assert_eq!(lines.len(), 4);
    assert!(renderer.contains("table: 1x1"));
    assert!(renderer.contains("list: 2 items"));
}

// ── MockHeadlessRenderer tests ──────────────────────────────────────────────

#[test]
fn mock_headless_renderer_captures_events() {
    let renderer = MockHeadlessRenderer::new();
    let mut buf = Vec::new();
    renderer
        .emit(
            &mut buf,
            HeadlessEvent::GateStart {
                name: "check".into(),
            },
        )
        .unwrap();
    renderer
        .emit(
            &mut buf,
            HeadlessEvent::GatePass {
                name: "check".into(),
                duration_ms: 42,
            },
        )
        .unwrap();
    assert_eq!(renderer.event_count(), 2);
}

// ── Assertion helper tests ──────────────────────────────────────────────────

#[test]
fn assert_file_exists_passes_for_existing_file() {
    let project = TempProject::new();
    project.write_file("hello.txt", "world");
    assert_file_exists(project.path(), "hello.txt");
}

#[test]
#[should_panic(expected = "expected file to exist")]
fn assert_file_exists_panics_for_missing_file() {
    let project = TempProject::new();
    assert_file_exists(project.path(), "nope.txt");
}

#[test]
fn assert_file_contains_passes_when_present() {
    let project = TempProject::new();
    project.write_file("data.txt", "hello world");
    assert_file_contains(project.path(), "data.txt", "world");
}

#[test]
#[should_panic(expected = "to contain")]
fn assert_file_contains_panics_when_absent() {
    let project = TempProject::new();
    project.write_file("data.txt", "hello");
    assert_file_contains(project.path(), "data.txt", "goodbye");
}
