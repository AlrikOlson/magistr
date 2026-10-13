use super::*;

// Pure type tests (gate_labels, gate_display, validation_result_*) now live
// in magistr-core::validate::tests. This file tests CLI-specific behavior:
// async execution, CLI extensions, and output truncation.

#[test]
fn gate_result_status_symbol_ext() {
    let pass = GateResult {
        gate: Gate::Check,
        passed: true,
        duration: Duration::from_secs(1),
        output: String::new(),
        command: "echo ok".into(),
    };
    assert_eq!(pass.status_symbol(), "✓");
    assert_eq!(pass.status_label(), "PASS");

    let fail = GateResult {
        gate: Gate::Test,
        passed: false,
        duration: Duration::from_secs(2),
        output: String::new(),
        command: "false".into(),
    };
    assert_eq!(fail.status_symbol(), "✗");
    assert_eq!(fail.status_label(), "FAIL");
}

#[tokio::test]
async fn run_gate_success() {
    let dir = std::env::current_dir().unwrap();
    let result = run_single_gate(Gate::Check, "echo hello", &dir).await;
    assert!(result.passed);
    assert!(result.output.contains("hello"));
    assert_eq!(result.command, "echo hello");
}

#[tokio::test]
async fn run_gate_failure() {
    let dir = std::env::current_dir().unwrap();
    let result = run_single_gate(Gate::Test, "false", &dir).await;
    assert!(!result.passed);
}

#[tokio::test]
async fn run_gate_empty_command_is_unconfigured() {
    let dir = std::env::current_dir().unwrap();
    let result = run_single_gate(Gate::Lint, "", &dir).await;
    assert!(!result.passed);
    assert!(result.output.contains("not configured"));
}

#[tokio::test]
async fn run_gates_with_config_all_pass() {
    let dir = std::env::current_dir().unwrap();
    let qg = QualityGate {
        check: "true".into(),
        test: "true".into(),
        lint: "true".into(),
    };
    let result = run_gates_with_config(&qg, &dir).await.unwrap();
    assert!(result.all_passed());
    assert_eq!(result.results.len(), 3);
}

#[tokio::test]
async fn run_gates_with_config_partial_fail() {
    let dir = std::env::current_dir().unwrap();
    let qg = QualityGate {
        check: "true".into(),
        test: "false".into(),
        lint: "true".into(),
    };
    let result = run_gates_with_config(&qg, &dir).await.unwrap();
    assert!(!result.all_passed());
    assert_eq!(result.passed_count(), 2);
    assert_eq!(result.failed_count(), 1);
}

#[test]
fn load_config_missing_dir() {
    let dir = std::path::PathBuf::from("/tmp/nonexistent-magistr-test-dir");
    let err = load_project_config(&dir).unwrap_err();
    assert!(err.to_string().contains("no .magistr/project.json"));
}

#[tokio::test]
async fn post_scaffold_check_all_pass() {
    let dir = std::env::current_dir().unwrap();
    let qg = QualityGate {
        check: "true".into(),
        test: "true".into(),
        lint: "true".into(),
    };
    let results = post_scaffold_check(&qg, &dir).await;
    assert_eq!(results.len(), 3);
    assert!(results.iter().all(|r| r.passed));
}

#[tokio::test]
async fn post_scaffold_check_partial_fail() {
    let dir = std::env::current_dir().unwrap();
    let qg = QualityGate {
        check: "true".into(),
        test: "false".into(),
        lint: "true".into(),
    };
    let results = post_scaffold_check(&qg, &dir).await;
    assert_eq!(results.len(), 3);
    let failed: Vec<_> = results.iter().filter(|r| !r.passed).collect();
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].gate, Gate::Test);
}

#[tokio::test]
async fn post_scaffold_check_skips_empty_commands() {
    let dir = std::env::current_dir().unwrap();
    let qg = QualityGate {
        check: "true".into(),
        test: String::new(),
        lint: "true".into(),
    };
    let results = post_scaffold_check(&qg, &dir).await;
    assert_eq!(results.len(), 2);
}

#[test]
fn post_scaffold_result_icons() {
    let pass = PostScaffoldResult {
        gate: Gate::Check,
        command: "true".into(),
        passed: true,
        output: String::new(),
        duration: Duration::from_millis(100),
    };
    assert!(pass.status_icon().contains("✓") || pass.status_icon().contains("OK"));

    let fail = PostScaffoldResult {
        gate: Gate::Test,
        command: "false".into(),
        passed: false,
        output: String::new(),
        duration: Duration::from_millis(200),
    };
    assert!(fail.status_icon().contains("✗") || fail.status_icon().contains("FAIL"));
}

#[tokio::test]
async fn run_gate_does_not_hang_on_stdin() {
    let dir = std::env::current_dir().unwrap();
    let result = run_single_gate(Gate::Check, "cat", &dir).await;
    assert!(result.passed);
    assert!(result.output.is_empty());
}

#[tokio::test]
async fn run_gate_captures_stderr() {
    let dir = std::env::current_dir().unwrap();
    let result = run_single_gate(Gate::Check, "echo err >&2", &dir).await;
    assert!(result.passed);
    assert!(result.output.contains("err"));
}

// ── Output truncation tests (BF1.6) ─────────────────────────────────────

#[test]
fn truncate_output_no_op_for_small_input() {
    let small = "hello";
    assert_eq!(truncate_output(small, 100), small);
}

#[test]
fn truncate_output_truncates_large_input() {
    let big = "x".repeat(500);
    let result = truncate_output(&big, 200);
    assert!(result.contains("[... truncated, 500 bytes total ...]"));
    assert!(result.len() < 500);
}

#[tokio::test]
async fn run_single_gate_truncates_large_output() {
    let dir = std::env::current_dir().unwrap();
    let cmd = format!("python3 -c \"print('a' * {})\"", MAX_OUTPUT_BYTES + 5_000);
    let result = run_single_gate(Gate::Check, &cmd, &dir).await;
    assert!(result.passed);
    assert!(
        result.output.len() <= MAX_OUTPUT_BYTES + 200,
        "output should be truncated: got {} bytes",
        result.output.len()
    );
    assert!(result.output.contains("[... truncated,"));
}
