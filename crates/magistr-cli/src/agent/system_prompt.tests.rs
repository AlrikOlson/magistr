use super::*;
use crate::types::{
    FormatConfig, ProjectMeta, QualityGate, SpecConfig, Stack, StackConfig, TestingConfig,
};

fn sample_config() -> ProjectConfig {
    ProjectConfig {
        project: ProjectMeta {
            name: "my-app".into(),
            description: "A sample project".into(),
            project_type: "cli".into(),
        },
        stack: StackConfig {
            language: "rust".into(),
            framework: "none".into(),
            package_manager: "cargo".into(),
            test_framework: "cargo-test".into(),
            linter: "clippy".into(),
            formatter: "rustfmt".into(),
        },
        quality_gate: QualityGate {
            check: "cargo check --all-targets".into(),
            test: "cargo test".into(),
            lint: "cargo clippy -- -D warnings".into(),
        },
        format: FormatConfig {
            extensions: vec!["rs".into()],
            command: "cargo fmt".into(),
            exclusions: vec![],
        },
        spec: Some(SpecConfig {
            file: "ROADMAP.md".into(),
            unchecked_pattern: r"^\s*- \[ \]".into(),
        }),
        testing: TestingConfig {
            companion_pattern: "tests/{name}.rs".into(),
            test_dir: "tests/".into(),
        },
        reference_repos: vec![],
        state_prefix: "my-app".into(),
        custom_gates: vec![],
        guards: Default::default(),
        hints: vec![],
        lifecycle_hooks: vec![],
        roadmap_backend: Default::default(),
        roadmap_exports: true,
        archetypes: vec![],
        topology: None,
        relationships: vec![],
    }
}

#[test]
fn format_prompt_includes_project_info() {
    let config = sample_config();
    let prompt = format_prompt(&config);

    assert!(prompt.contains("**Project:** my-app"));
    assert!(prompt.contains("**Description:** A sample project"));
    assert!(prompt.contains("**Type:** cli"));
}

#[test]
fn format_prompt_includes_stack() {
    let config = sample_config();
    let prompt = format_prompt(&config);

    assert!(prompt.contains("Language: rust"));
    assert!(prompt.contains("Package Manager: cargo"));
    assert!(prompt.contains("Linter: clippy"));
}

#[test]
fn format_prompt_omits_framework_none() {
    let config = sample_config();
    let prompt = format_prompt(&config);

    assert!(!prompt.contains("Framework: none"));
}

#[test]
fn format_prompt_includes_framework_when_set() {
    let mut config = sample_config();
    config.stack.framework = "axum".into();
    let prompt = format_prompt(&config);

    assert!(prompt.contains("Framework: axum"));
}

#[test]
fn format_prompt_includes_quality_gates() {
    let config = sample_config();
    let prompt = format_prompt(&config);

    assert!(prompt.contains("`cargo check --all-targets`"));
    assert!(prompt.contains("`cargo test`"));
    assert!(prompt.contains("`cargo clippy -- -D warnings`"));
}

#[test]
fn format_prompt_includes_spec() {
    let config = sample_config();
    let prompt = format_prompt(&config);

    assert!(prompt.contains("`ROADMAP.md`"));
}

#[test]
fn format_prompt_does_not_contain_hardcoded_tool_guidance() {
    let config = sample_config();
    let prompt = format_prompt(&config);

    // Tool guidance is now in .claude/rules/tools.md, not hardcoded in format_prompt()
    assert!(
        !prompt.contains("## Tool Preferences"),
        "tool preferences should not be hardcoded in format_prompt"
    );
    assert!(
        !prompt.contains("## Workflow Tools"),
        "workflow tools should not be hardcoded in format_prompt"
    );
}

#[test]
fn build_system_prompt_returns_none_for_missing_dir() {
    let result = build_system_prompt(Path::new("/nonexistent/path"));
    assert!(result.is_none());
}

#[test]
fn agent_context_supplies_recording_guidance_without_rewriting_project_rules() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".magistr")).unwrap();
    std::fs::create_dir_all(dir.path().join(".claude/rules")).unwrap();
    std::fs::write(
        dir.path().join(".magistr/project.json"),
        serde_json::to_vec(&sample_config()).unwrap(),
    )
    .unwrap();
    let rules = dir.path().join(".claude/rules/tools.md");
    let guidance = magistr_engine::prompt::PromptConfig::REASONING_GUIDANCE;
    for content in [
        None,
        Some("Custom project instructions: preserve this text.".to_owned()),
        Some(format!("Custom introduction.\n{guidance}")),
    ] {
        if let Some(content) = &content {
            std::fs::write(&rules, content).unwrap();
        }
        let context = crate::agent::AgentConfig::new("Continue the work", dir.path())
            .with_project_context(dir.path());
        let prompt = context.system_prompt.unwrap();
        assert!(!guidance.is_empty());
        assert_eq!(prompt.matches(guidance).count(), 1);
        match content {
            Some(content) => {
                assert!(prompt.contains(&content));
                assert_eq!(std::fs::read_to_string(&rules).unwrap(), content);
            }
            None => assert!(!rules.exists()),
        }
    }
}

#[test]
fn format_prompt_includes_hints_when_present() {
    let mut config = sample_config();
    config.hints = vec![
        "Use mcp-image for asset generation".into(),
        "Transparent PNGs are auto-extracted by hook".into(),
    ];
    let prompt = format_prompt(&config);

    assert!(prompt.contains("## Project Hints"));
    assert!(prompt.contains("Use mcp-image for asset generation"));
    assert!(prompt.contains("Transparent PNGs are auto-extracted by hook"));
}

#[test]
fn format_prompt_omits_hints_when_empty() {
    let config = sample_config();
    let prompt = format_prompt(&config);

    assert!(!prompt.contains("## Project Hints"));
}

#[test]
fn format_prompt_omits_empty_description() {
    let mut config = sample_config();
    config.project.description = String::new();
    let prompt = format_prompt(&config);

    assert!(!prompt.contains("**Description:**"));
}

fn sample_stack() -> Stack {
    Stack {
        project_name: "test-project".into(),
        language: crate::types::Language::Rust,
        framework: crate::types::Framework::Axum,
        package_manager: crate::types::PackageManager::Cargo,
        test_framework: crate::types::TestFramework::CargoTest,
        linter: crate::types::Linter::Clippy,
        formatter: crate::types::Formatter::Rustfmt,
        workspace: crate::types::Workspace::None,
        ci_provider: crate::types::CiProvider::None,
        extra_detections: Vec::new(),
    }
}

#[test]
fn init_prompt_includes_stack_json() {
    let stack = sample_stack();
    let prompt = build_init_prompt(&stack, "/home/user/myproject");

    assert!(prompt.contains("test-project"));
    assert!(prompt.contains("rust"));
    assert!(prompt.contains("axum"));
    assert!(prompt.contains("cargo"));
}

#[test]
fn init_prompt_includes_project_dir() {
    let stack = sample_stack();
    let prompt = build_init_prompt(&stack, "/home/user/myproject");

    assert!(prompt.contains("/home/user/myproject"));
}

#[test]
fn init_prompt_includes_interview_instructions() {
    let stack = sample_stack();
    let prompt = build_init_prompt(&stack, "/tmp/test");

    assert!(prompt.contains("interview"));
    assert!(prompt.contains("project.json"));
    assert!(prompt.contains("settings.json"));
    assert!(prompt.contains("workflow.md"));
    assert!(prompt.contains("conventions.md"));
}

#[test]
fn init_prompt_includes_config_schema() {
    let stack = sample_stack();
    let prompt = build_init_prompt(&stack, "/tmp/test");

    assert!(prompt.contains("quality_gate"));
    assert!(prompt.contains("state_prefix"));
    assert!(prompt.contains("companion_pattern"));
}

#[test]
fn build_system_prompt_includes_memory_when_present() {
    let dir = tempfile::tempdir().unwrap();
    let claude_dir = dir.path().join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    let magistr_dir = dir.path().join(".magistr");
    std::fs::create_dir_all(&magistr_dir).unwrap();

    // Write a minimal project.json
    let config = sample_config();
    let config_json = serde_json::to_string_pretty(&config).unwrap();
    std::fs::write(magistr_dir.join("project.json"), &config_json).unwrap();

    // Write a memory.md file
    let memory_content = "# Project Memory — my-app\n\n## Patterns\n\n- Uses builder pattern for config structs\n- Prefers iterators over manual loops\n";
    std::fs::write(magistr_dir.join("memory.md"), memory_content).unwrap();

    let prompt = build_system_prompt(dir.path()).unwrap();

    assert!(
        prompt.contains("Project Memory"),
        "prompt should include project memory section"
    );
    assert!(
        prompt.contains("builder pattern"),
        "prompt should include memory entries"
    );
}

#[test]
fn format_prompt_includes_lifecycle_hooks_when_present() {
    use magistr_engine::{HookAction, LifecycleEvent, LifecycleHook, WorkflowPhase};

    let mut config = sample_config();
    config.lifecycle_hooks = vec![LifecycleHook {
        event: LifecycleEvent::OnEnter(WorkflowPhase::Verifying),
        action: HookAction::McpTool {
            tool: "magistr_format".into(),
            arguments: serde_json::Value::Null,
        },
        description: Some("Auto-format before verify — do not call manually".into()),
        on_failure: magistr_engine::OnFailure::Continue,
    }];
    let prompt = format_prompt(&config);

    assert!(
        prompt.contains("## Lifecycle Hooks"),
        "prompt should include lifecycle hooks section"
    );
    assert!(
        prompt.contains("magistr_format"),
        "prompt should list the tool name"
    );
    assert!(
        prompt.contains("Auto-format before verify"),
        "prompt should include the hook description"
    );
    assert!(
        prompt.contains("do not call"),
        "prompt should warn agent not to call manually"
    );
}

#[test]
fn format_prompt_omits_lifecycle_hooks_when_empty() {
    let config = sample_config();
    let prompt = format_prompt(&config);

    assert!(
        !prompt.contains("## Lifecycle Hooks"),
        "prompt should not include lifecycle hooks section when empty"
    );
}

#[test]
fn format_prompt_lifecycle_hooks_grouped_by_event() {
    use magistr_engine::{HookAction, LifecycleEvent, LifecycleHook, WorkflowPhase};

    let mut config = sample_config();
    config.lifecycle_hooks = vec![
        LifecycleHook {
            event: LifecycleEvent::OnEnter(WorkflowPhase::Verifying),
            action: HookAction::McpTool {
                tool: "magistr_format".into(),
                arguments: serde_json::Value::Null,
            },
            description: None,
            on_failure: magistr_engine::OnFailure::Continue,
        },
        LifecycleHook {
            event: LifecycleEvent::OnEnter(WorkflowPhase::Verifying),
            action: HookAction::McpTool {
                tool: "magistr_lint".into(),
                arguments: serde_json::Value::Null,
            },
            description: None,
            on_failure: magistr_engine::OnFailure::FailFast,
        },
        LifecycleHook {
            event: LifecycleEvent::OnIterationStart,
            action: HookAction::McpTool {
                tool: "scope_search".into(),
                arguments: serde_json::Value::Null,
            },
            description: Some("Pre-scan codebase".into()),
            on_failure: magistr_engine::OnFailure::Warn,
        },
    ];
    let prompt = format_prompt(&config);

    // Both event groups should appear
    assert!(
        prompt.contains("on_enter(Verifying)"),
        "prompt should show on_enter(Verifying) event group"
    );
    assert!(
        prompt.contains("on_iteration_start"),
        "prompt should show on_iteration_start event group"
    );

    // Tools should appear under their event groups
    assert!(prompt.contains("magistr_format"));
    assert!(prompt.contains("magistr_lint"));
    assert!(prompt.contains("scope_search"));
    assert!(prompt.contains("Pre-scan codebase"));
}

#[test]
fn build_system_prompt_works_without_memory() {
    let dir = tempfile::tempdir().unwrap();
    let claude_dir = dir.path().join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    let magistr_dir = dir.path().join(".magistr");
    std::fs::create_dir_all(&magistr_dir).unwrap();

    // Write project.json but no memory.md
    let config = sample_config();
    let config_json = serde_json::to_string_pretty(&config).unwrap();
    std::fs::write(magistr_dir.join("project.json"), &config_json).unwrap();

    let prompt = build_system_prompt(dir.path()).unwrap();

    assert!(
        !prompt.contains("Project Memory"),
        "prompt should not include memory section when no memory.md exists"
    );
}
