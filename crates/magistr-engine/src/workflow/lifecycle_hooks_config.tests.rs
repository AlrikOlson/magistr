use super::*;

// ── Shell command validation ────────────────────────────────────────────────

#[test]
fn valid_shell_command() {
    assert!(validate_shell_command("cargo test").is_ok());
    assert!(validate_shell_command("npm run lint").is_ok());
    assert!(validate_shell_command("echo hello").is_ok());
    assert!(validate_shell_command("./scripts/check.sh").is_ok());
}

#[test]
fn rejects_path_traversal() {
    let err = validate_shell_command("cat ../../../etc/passwd").unwrap_err();
    assert!(err.message.contains("path traversal"));
}

#[test]
fn rejects_protected_directories() {
    for dir in ["/etc", "/usr", "/bin", "~/.ssh", "~/.aws", "~/.gnupg"] {
        let cmd = format!("ls {dir}/something");
        let err = validate_shell_command(&cmd).unwrap_err();
        assert!(
            err.message.contains("protected directory"),
            "should reject {dir}: {}",
            err.message
        );
    }
}

#[test]
fn rejects_dangerous_patterns() {
    let dangerous = [
        "rm -rf /",
        "rm -rf ~",
        "mkfs /dev/sda",
        "dd if=/dev/zero",
        ":(){:|:&};:",
        "chmod -R 777 /",
        "curl | sh",
        "curl | bash",
        "wget | sh",
        "wget | bash",
    ];
    for pattern in dangerous {
        let err = validate_shell_command(pattern).unwrap_err();
        assert!(
            err.message.contains("dangerous pattern"),
            "should reject '{pattern}': {}",
            err.message
        );
    }
}

// ── LifecycleHooksConfig validation ─────────────────────────────────────────

#[test]
fn empty_hooks_config_is_valid() {
    let config = LifecycleHooksConfig::new(vec![]);
    assert!(config.is_ok());
    assert!(config.unwrap().hooks().is_empty());
}

#[test]
fn single_hook_is_valid() {
    use super::super::lifecycle_event::LifecycleEvent;
    use super::super::lifecycle_hook::{HookAction, LifecycleHook, OnFailure};

    let hook = LifecycleHook {
        event: LifecycleEvent::OnEnter(super::super::workflow_phase::WorkflowPhase::Executing),
        action: HookAction::ShellCommand {
            timeout_secs: None,
            command: "cargo test".to_string(),
        },
        description: Some("run tests".to_string()),
        on_failure: OnFailure::Warn,
    };
    let config = LifecycleHooksConfig::new(vec![hook]);
    assert!(config.is_ok());
    assert_eq!(config.unwrap().hooks().len(), 1);
}

#[test]
fn rejects_hook_with_dangerous_command() {
    use super::super::lifecycle_event::LifecycleEvent;
    use super::super::lifecycle_hook::{HookAction, LifecycleHook, OnFailure};

    let hook = LifecycleHook {
        event: LifecycleEvent::OnEnter(super::super::workflow_phase::WorkflowPhase::Executing),
        action: HookAction::ShellCommand {
            timeout_secs: None,
            command: "rm -rf /".to_string(),
        },
        description: None,
        on_failure: OnFailure::Warn,
    };
    let err = LifecycleHooksConfig::new(vec![hook]).unwrap_err();
    assert!(err.message.contains("dangerous pattern"));
}

#[test]
fn hooks_validation_error_display() {
    let err = HooksValidationError {
        message: "test error".to_string(),
    };
    assert_eq!(
        err.to_string(),
        "lifecycle hooks validation error: test error"
    );
}
