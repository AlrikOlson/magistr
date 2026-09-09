use super::*;

// ── classify_command ───────────────────────────────────────────────────

#[test]
fn classify_compound_rust_gate() {
    let m = GatePatterns::classify_command(
        "cargo check && cargo nextest run && cargo clippy -- -D warnings",
    );
    assert!(m.has_check);
    assert!(m.has_test);
    assert!(m.has_lint);
}

#[test]
fn classify_test_only() {
    let m = GatePatterns::classify_command("cargo test --lib");
    assert!(!m.has_check);
    assert!(m.has_test);
    assert!(!m.has_lint);
}

#[test]
fn classify_lint_only() {
    let m = GatePatterns::classify_command("cargo clippy --all-targets -- -D warnings");
    assert!(!m.has_check);
    assert!(!m.has_test);
    assert!(m.has_lint);
}

#[test]
fn classify_check_only() {
    let m = GatePatterns::classify_command("cargo check --all-targets");
    assert!(m.has_check);
    assert!(!m.has_test);
    assert!(!m.has_lint);
}

#[test]
fn classify_no_gate() {
    let m = GatePatterns::classify_command("echo hello");
    assert!(!m.any());
}

#[test]
fn classify_node_project() {
    let m = GatePatterns::classify_command("npm test && npm run lint");
    assert!(m.has_test);
    assert!(m.has_lint);
}

#[test]
fn classify_go_project() {
    let m = GatePatterns::classify_command("go build ./... && go test ./... && golangci-lint run");
    assert!(m.has_check);
    assert!(m.has_test);
    assert!(m.has_lint);
}

#[test]
fn classify_python_project() {
    let m = GatePatterns::classify_command("pytest && python -m mypy src/");
    assert!(m.has_test);
    assert!(m.has_lint);
}

#[test]
fn classify_tsc_is_check() {
    let m = GatePatterns::classify_command("tsc --noEmit");
    assert!(m.has_check);
}

#[test]
fn classify_nextest() {
    let m = GatePatterns::classify_command("cargo nextest run");
    assert!(m.has_test);
}

#[test]
fn classify_llvm_cov() {
    let m = GatePatterns::classify_command("cargo llvm-cov nextest --fail-under-lines 80");
    assert!(m.has_test);
}

// ── is_quality_gate ────────────────────────────────────────────────────

#[test]
fn quality_gate_with_config() {
    let gate = Some(QualityGate {
        check: "cargo check --all-targets".to_string(),
        test: "cargo nextest run".to_string(),
        lint: "cargo clippy --all-targets -- -D warnings".to_string(),
    });
    assert!(GatePatterns::is_quality_gate(
        "cargo clippy --all-targets -- -D warnings",
        &gate
    ));
    assert!(!GatePatterns::is_quality_gate("cargo test --lib", &gate));
}

#[test]
fn quality_gate_fallback_heuristics() {
    assert!(GatePatterns::is_quality_gate(
        "cargo clippy -- -D warnings",
        &None
    ));
    assert!(GatePatterns::is_quality_gate("npm run lint", &None));
    assert!(!GatePatterns::is_quality_gate("cargo test", &None));
}

// ── is_test_only ───────────────────────────────────────────────────────

#[test]
fn test_only_detection() {
    assert!(GatePatterns::is_test_only("cargo test --lib"));
    assert!(GatePatterns::is_test_only("npm test"));
    assert!(GatePatterns::is_test_only("pytest tests/"));
    assert!(!GatePatterns::is_test_only("cargo test && cargo clippy"));
    assert!(!GatePatterns::is_test_only("echo hello"));
}
