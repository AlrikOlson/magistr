use super::*;
use crate::workflow::{LifecycleEvent, WorkflowPhase};

#[test]
fn rust_preset_has_format_and_gate() {
    let hooks = preset_for_language("rust");
    assert_eq!(hooks.len(), 2);
    assert_eq!(hooks[0].display_name(), "magistr_format");
    assert_eq!(
        hooks[0].event,
        LifecycleEvent::OnEnter(WorkflowPhase::Verifying)
    );
    assert_eq!(hooks[1].display_name(), "magistr_gate");
    assert_eq!(
        hooks[1].event,
        LifecycleEvent::OnExit(WorkflowPhase::Verifying)
    );
    assert_eq!(hooks[1].on_failure, OnFailure::FailFast);
}

#[test]
fn node_preset_has_lint() {
    let hooks = preset_for_language("typescript");
    assert_eq!(hooks.len(), 1);
    assert_eq!(hooks[0].display_name(), "magistr_lint");
    assert_eq!(
        hooks[0].event,
        LifecycleEvent::OnEnter(WorkflowPhase::Verifying)
    );
}

#[test]
fn javascript_uses_node_preset() {
    let hooks = preset_for_language("javascript");
    assert_eq!(hooks.len(), 1);
    assert_eq!(hooks[0].display_name(), "magistr_lint");
}

#[test]
fn python_preset_has_format_on_execute() {
    let hooks = preset_for_language("python");
    assert_eq!(hooks.len(), 1);
    assert_eq!(hooks[0].display_name(), "magistr_format");
    assert_eq!(
        hooks[0].event,
        LifecycleEvent::OnEnter(WorkflowPhase::Executing)
    );
}

#[test]
fn unknown_language_returns_empty() {
    assert!(preset_for_language("cobol").is_empty());
    assert!(preset_for_language("").is_empty());
    assert!(preset_for_language("unknown").is_empty());
}

#[test]
fn case_insensitive_matching() {
    assert_eq!(preset_for_language("Rust").len(), 2);
    assert_eq!(preset_for_language("PYTHON").len(), 1);
    assert_eq!(preset_for_language("TypeScript").len(), 1);
}

#[test]
fn all_presets_produce_valid_hooks() {
    use crate::workflow::LifecycleHooksConfig;

    for lang in &["rust", "typescript", "javascript", "python"] {
        let hooks = preset_for_language(lang);
        let result = LifecycleHooksConfig::new(hooks);
        assert!(
            result.is_ok(),
            "preset for {lang} should produce valid hooks config"
        );
    }
}
