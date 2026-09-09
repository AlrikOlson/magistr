use super::*;

const RUSTC_RENDERED: &str = include_str!("../fixtures/gate_rows/rustc_rendered.txt");
const CARGO_JSON: &str = include_str!("../fixtures/gate_rows/cargo_json.txt");
const NEXTEST_FAIL: &str = include_str!("../fixtures/gate_rows/nextest_fail.txt");
const LIBTEST_FAIL: &str = include_str!("../fixtures/gate_rows/libtest_fail.txt");
const JUNIT: &str = include_str!("../fixtures/gate_rows/junit.xml");
const GENERIC: &str = include_str!("../fixtures/gate_rows/generic.txt");

fn row(
    severity: GateRowSeverity,
    code: Option<&str>,
    message: &str,
    file: Option<&str>,
    line: Option<u32>,
    column: Option<u32>,
) -> GateFailureRow {
    GateFailureRow {
        severity,
        code: code.map(str::to_owned),
        message: message.to_owned(),
        file: file.map(str::to_owned),
        line,
        column,
    }
}

#[test]
fn rendered_rustc_and_clippy_diagnostics_become_rows_and_summaries_are_dropped() {
    let rows = parse_gate_rows("cargo clippy -- -D warnings", RUSTC_RENDERED);
    assert_eq!(
        rows,
        vec![
            row(
                GateRowSeverity::Error,
                Some("E0308"),
                "mismatched types",
                Some("src/tui/app.rs"),
                Some(1756),
                Some(27),
            ),
            row(
                GateRowSeverity::Warning,
                Some("unused_variables"),
                "unused variable: `elapsed`",
                Some("src/tui/dashboard/layout.rs"),
                Some(1345),
                Some(25),
            ),
            row(
                GateRowSeverity::Warning,
                Some("clippy::if_same_then_else"),
                "this `if` has identical blocks",
                Some("src/tui/sidebar/task_list.rs"),
                Some(280),
                Some(17),
            ),
        ]
    );
    assert_eq!(
        rows[0].location().as_deref(),
        Some("src/tui/app.rs:1756:27")
    );
}

#[test]
fn json_compiler_messages_win_and_dedupe_against_the_rendered_form() {
    let rows = parse_gate_rows("cargo check --message-format=json", CARGO_JSON);
    assert_eq!(
        rows,
        vec![
            row(
                GateRowSeverity::Error,
                Some("E0425"),
                "cannot find value `rows` in this scope",
                Some("src/mcp_tools/gate.rs"),
                Some(75),
                Some(9),
            ),
            row(
                GateRowSeverity::Warning,
                Some("unused_imports"),
                "unused import: `std::path::Path`",
                Some("src/mcp_tools/gate.rs"),
                Some(3),
                Some(5),
            ),
        ],
        "aborting/could-not-compile summaries dropped, backslash path normalized"
    );
}

#[test]
fn nextest_failures_carry_the_panic_location_and_skip_retries() {
    let rows = parse_gate_rows("cargo nextest run", NEXTEST_FAIL);
    assert_eq!(
        rows,
        vec![
            row(
                GateRowSeverity::TestFailure,
                None,
                "magistr-cli tui::stream::viewport::tests::viewport_window_performance_100k_lines: \
                 viewport_window took 593ms for 100K lines (should be <500ms)",
                Some("src/tui/stream/viewport.tests.rs"),
                Some(233),
                Some(5),
            ),
            row(
                GateRowSeverity::TestFailure,
                None,
                "magistr-cli::next_iteration_integration \
                 headless_iteration_is_guarded_on_ship_state_and_cross_referenced: \
                 assertion failed: `(left == right)`",
                Some("tests/next_iteration_integration.rs"),
                Some(83),
                Some(5),
            ),
        ]
    );
}

#[test]
fn libtest_failures_pair_the_failed_line_with_its_stdout_block() {
    let rows = parse_gate_rows("cargo test -p magistr-core", LIBTEST_FAIL);
    assert_eq!(
        rows,
        vec![row(
            GateRowSeverity::TestFailure,
            None,
            "workflow::iteration::tests::native_prompt_names_the_next_chunk: \
             assertion failed: prompt.contains(\"guards — Chunk guards\")",
            Some("crates/magistr-core/src/workflow/iteration.tests.rs"),
            Some(427),
            Some(5),
        )]
    );
}

#[test]
fn a_failed_build_under_a_test_command_reports_the_compiler_error() {
    let rows = parse_gate_rows("cargo nextest run", RUSTC_RENDERED);
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].severity, GateRowSeverity::Error);
}

#[test]
fn junit_failures_and_errors_become_rows() {
    let rows = junit_failure_rows(JUNIT);
    assert_eq!(
        rows,
        vec![
            row(
                GateRowSeverity::TestFailure,
                None,
                "magistr-cli::tui::stream::viewport::tests::viewport_window_performance_100k_lines: \
                 viewport_window took 593ms for 100K lines (should be <500ms)",
                None,
                None,
                None,
            ),
            row(
                GateRowSeverity::TestFailure,
                None,
                "magistr-cli::next_iteration_integration::headless_iteration_is_guarded: \
                 assertion failed: `(left == right)`",
                None,
                None,
                None,
            ),
        ]
    );
    assert!(junit_failure_rows("<testsuites/>").is_empty());
}

#[test]
fn generic_lines_cover_eslint_tsc_pytest_and_mypy() {
    let rows = parse_gate_rows("npm run lint", GENERIC);
    assert_eq!(
        rows,
        vec![
            row(
                GateRowSeverity::Error,
                None,
                "'snapshot' is never reassigned. Use 'const' instead. (prefer-const)",
                Some("src/lib/stores/chunks.svelte.ts"),
                Some(42),
                Some(7),
            ),
            row(
                GateRowSeverity::Error,
                Some("TS2322"),
                "Type 'string' is not assignable to type 'number'.",
                Some("src/routes/+page.svelte"),
                Some(12),
                Some(5),
            ),
            row(
                GateRowSeverity::Error,
                None,
                "AssertionError: expected 3 items, got 2",
                Some("tests/test_models.py"),
                Some(57),
                None,
            ),
            row(
                GateRowSeverity::Error,
                None,
                "Incompatible return value type (got \"str\", expected \"int\")  [return-value]",
                Some("lib/utils.py"),
                Some(12),
                None,
            ),
            row(
                GateRowSeverity::Warning,
                None,
                "Unexpected console statement (no-console)",
                Some("src/lib/components/GateResultsBar.svelte"),
                Some(88),
                Some(11),
            ),
        ],
        "errors first, warnings last; the URL line is not a file"
    );
}

#[test]
fn clean_output_yields_no_rows() {
    assert!(parse_gate_rows("cargo check", "    Finished `dev` profile in 0.2s\n").is_empty());
    assert!(
        parse_gate_rows(
            "cargo nextest run",
            "     Summary [ 1.0s] 3 tests run: 3 passed\n"
        )
        .is_empty()
    );
    assert!(parse_gate_rows("true", "").is_empty());
}

#[test]
fn rows_round_trip_through_json_without_empty_optionals() {
    let r = row(
        GateRowSeverity::Warning,
        None,
        "m",
        Some("a.rs"),
        Some(1),
        None,
    );
    let json = serde_json::to_value(&r).unwrap();
    assert_eq!(json["severity"], "warning");
    assert!(json.get("code").is_none());
    assert!(json.get("column").is_none());
    let back: GateFailureRow = serde_json::from_value(json).unwrap();
    assert_eq!(back, r);
}

#[test]
fn location_without_a_file_is_none() {
    let r = row(GateRowSeverity::TestFailure, None, "t: x", None, None, None);
    assert_eq!(r.location(), None);
    let r = row(GateRowSeverity::Error, None, "m", Some("a.rs"), None, None);
    assert_eq!(r.location().as_deref(), Some("a.rs"));
}
