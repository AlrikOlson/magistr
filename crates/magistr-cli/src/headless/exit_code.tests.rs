use super::*;

#[test]
fn exit_code_values() {
    assert_eq!(ExitCode::Success as u8, 0);
    assert_eq!(ExitCode::GatesFailed as u8, 1);
    assert_eq!(ExitCode::RuntimeError as u8, 2);
    assert_eq!(ExitCode::BudgetExceeded as u8, 3);
}

#[test]
fn exit_code_to_process_exit_code() {
    let code: std::process::ExitCode = ExitCode::Success.into();
    assert_eq!(code, std::process::ExitCode::SUCCESS);
}

#[test]
fn exit_code_display() {
    assert_eq!(ExitCode::Success.to_string(), "success (0)");
    assert_eq!(ExitCode::GatesFailed.to_string(), "gates failed (1)");
    assert_eq!(ExitCode::RuntimeError.to_string(), "runtime error (2)");
    assert_eq!(ExitCode::BudgetExceeded.to_string(), "budget exceeded (3)");
}

#[test]
fn exit_code_is_success() {
    assert!(ExitCode::Success.is_success());
    assert!(!ExitCode::GatesFailed.is_success());
    assert!(!ExitCode::RuntimeError.is_success());
    assert!(!ExitCode::BudgetExceeded.is_success());
}
