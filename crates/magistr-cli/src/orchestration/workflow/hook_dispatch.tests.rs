use magistr_engine::{HookAction, HookDispatcher};

use super::ProcessHookDispatcher;

#[tokio::test]
async fn unknown_tool_succeeds_with_skip_message() {
    let dispatcher = ProcessHookDispatcher::empty(std::path::Path::new("/tmp"));
    let action = HookAction::McpTool {
        tool: "unknown_tool".into(),
        arguments: serde_json::Value::Null,
    };
    let result = dispatcher.dispatch(&action).await;
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.message.unwrap().contains("skipped"));
}

#[tokio::test]
async fn format_command_runs_successfully() {
    let dir = tempfile::tempdir().unwrap();
    let config = crate::types::ProjectConfig {
        project: crate::types::ProjectMeta {
            id: None,
            name: "test".into(),
            description: String::new(),
            project_type: String::new(),
        },
        stack: crate::types::StackConfig {
            language: "rust".into(),
            framework: "none".into(),
            package_manager: "cargo".into(),
            test_framework: "cargo test".into(),
            linter: "clippy".into(),
            formatter: "rustfmt".into(),
        },
        quality_gate: crate::types::QualityGate {
            check: "true".into(),
            test: "true".into(),
            lint: "true".into(),
        },
        format: crate::types::FormatConfig {
            command: "echo formatted".into(),
            extensions: vec!["rs".into()],
            exclusions: vec![],
        },
        spec: Some(crate::types::SpecConfig {
            file: "ROADMAP.md".into(),
            unchecked_pattern: "- [ ]".into(),
        }),
        testing: crate::types::TestingConfig {
            companion_pattern: "{name}.tests.rs".into(),
            test_dir: "tests".into(),
        },
        reference_repos: vec![],
        state_prefix: ".magistr".into(),
        custom_gates: vec![],
        guards: Default::default(),
        hints: vec![],
        lifecycle_hooks: vec![],
        roadmap_backend: Default::default(),
        roadmap_exports: true,
        archetypes: vec![],
        topology: None,
        relationships: vec![],
    };

    let dispatcher = ProcessHookDispatcher::new(dir.path(), &config);
    let action = HookAction::McpTool {
        tool: "magistr_format".into(),
        arguments: serde_json::Value::Null,
    };
    let result = dispatcher.dispatch(&action).await;
    assert!(result.is_ok());
    let output = result.unwrap();
    assert_eq!(output.message.as_deref(), Some("formatted"));
}

#[tokio::test]
async fn shell_command_runs_directly() {
    let dir = tempfile::tempdir().unwrap();
    let dispatcher = ProcessHookDispatcher::empty(dir.path());
    let action = HookAction::ShellCommand {
        command: "echo hello-shell".into(),
        timeout_secs: None,
    };
    let result = dispatcher.dispatch(&action).await;
    assert!(result.is_ok());
    let output = result.unwrap();
    assert_eq!(output.message.as_deref(), Some("hello-shell"));
}

#[tokio::test]
async fn shell_command_captures_failure() {
    let dir = tempfile::tempdir().unwrap();
    let dispatcher = ProcessHookDispatcher::empty(dir.path());
    let action = HookAction::ShellCommand {
        command: "exit 1".into(),
        timeout_secs: None,
    };
    let result = dispatcher.dispatch(&action).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().message.contains("exit 1"));
}

#[tokio::test]
async fn shell_command_timeout() {
    let dir = tempfile::tempdir().unwrap();
    let dispatcher = ProcessHookDispatcher::empty(dir.path());
    let action = HookAction::ShellCommand {
        command: "sleep 10".into(),
        timeout_secs: Some(1),
    };
    let result = dispatcher.dispatch(&action).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().message.contains("timed out"));
}
