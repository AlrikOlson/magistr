use std::{path::PathBuf, sync::Mutex, time::Duration};

use super::*;
use crate::{
    config::ResolveOptions,
    contracts::{Recovery, TruncationReason},
    execution::{
        DiagnosticLevel, RunFailure, RunFuture, cargo_diagnostics::extract_cargo_diagnostics,
        cargo_subcommand::cargo_subcommand,
    },
};

/// An executor that answers a fixed execution and records what it was asked.
struct Scripted {
    answer: Result<RunExecution, WorkspaceError>,
    seen: Mutex<Vec<(Vec<String>, RunLimits)>>,
}

impl Scripted {
    fn new(answer: RunExecution) -> Self {
        Self {
            answer: Ok(answer),
            seen: Mutex::new(Vec::new()),
        }
    }

    fn failing(error: WorkspaceError) -> Self {
        Self {
            answer: Err(error),
            seen: Mutex::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<(Vec<String>, RunLimits)> {
        self.seen.lock().expect("scripted executor").clone()
    }
}

impl RunExecutor for Scripted {
    fn execute(
        &self,
        argv: Vec<String>,
        limits: RunLimits,
        _cancel: CancellationToken,
    ) -> RunFuture<'_, Result<RunExecution, WorkspaceError>> {
        self.seen
            .lock()
            .expect("scripted executor")
            .push((argv, limits));
        let answer = self.answer.clone();
        Box::pin(async move { answer })
    }
}

struct FakeLane {
    name: &'static str,
    answer: Result<Option<RunReport>, WorkspaceError>,
}

impl PluginRunnerLane for FakeLane {
    fn name(&self) -> &str {
        self.name
    }

    fn parse(
        &self,
        _output: RunOutput,
    ) -> RunFuture<'_, Result<Option<RunReport>, WorkspaceError>> {
        let answer = self.answer.clone();
        Box::pin(async move { answer })
    }
}

fn workspace(commands: &[(&str, &str)]) -> (tempfile::TempDir, WorkspacePolicy) {
    let dir = tempfile::tempdir().expect("workspace");
    let mut toml = String::from("[commands]\n");
    for (name, line) in commands {
        toml.push_str(&format!("{name} = {line:?}\n"));
    }
    std::fs::create_dir_all(dir.path().join(".magistr")).unwrap();
    std::fs::write(dir.path().join(".magistr/workspace.toml"), toml).expect("declare commands");
    let policy = WorkspacePolicy::resolve(&ResolveOptions::new(dir.path())).expect("policy");
    (dir, policy)
}

fn ok(over: RunExecution) -> RunExecution {
    over
}

fn exited(code: i32, stdout: &str, stderr: &str) -> RunExecution {
    RunExecution {
        exit_code: Some(code),
        duration_ms: 7,
        stdout: stdout.to_owned(),
        stderr: stderr.to_owned(),
        ..RunExecution::default()
    }
}

async fn run_with(
    commands: &[(&str, &str)],
    executor: &Scripted,
    input: RunInput,
) -> WorkspaceResponse<RunRow> {
    let (_dir, policy) = workspace(commands);
    WorkspaceRun::new(&policy, executor)
        .run(&input, CancellationToken::new())
        .await
}

fn row(response: &WorkspaceResponse<RunRow>) -> &RunRow {
    match response {
        WorkspaceResponse::Success { results, .. } => &results.rows()[0],
        WorkspaceResponse::Failure { error, .. } => panic!("expected ok, got {error}"),
    }
}

fn failure(response: &WorkspaceResponse<RunRow>) -> &WorkspaceError {
    match response {
        WorkspaceResponse::Failure { error, .. } => error,
        WorkspaceResponse::Success { .. } => panic!("expected an error envelope"),
    }
}

fn notes(response: &WorkspaceResponse<RunRow>) -> Vec<String> {
    match response {
        WorkspaceResponse::Success { notes, .. } | WorkspaceResponse::Failure { notes, .. } => {
            notes.clone().unwrap_or_default()
        }
    }
}

fn truncation(response: &WorkspaceResponse<RunRow>) -> Option<(usize, String)> {
    match response {
        WorkspaceResponse::Success { results, .. } => results
            .truncation()
            .map(|cut| (cut.dropped(), cut.cursor().to_owned())),
        WorkspaceResponse::Failure { .. } => None,
    }
}

fn report(row: &RunRow) -> (&str, &[RunFailure], Option<&str>) {
    match row {
        RunRow::Report {
            adapter,
            failures,
            summary,
            ..
        } => (adapter, failures, summary.as_deref()),
        other => panic!("expected a report row, got {other:?}"),
    }
}

fn diagnostic_row(row: &RunRow) -> (&str, &[Diagnostic], &str) {
    match row {
        RunRow::Diagnostics {
            extractor,
            diagnostics,
            summary,
            ..
        } => (extractor, diagnostics, summary),
        other => panic!("expected a diagnostics row, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// The allowlist and the no-shell guard (retained acceptance #1 and #2)

#[tokio::test]
async fn an_undeclared_command_answers_command_not_declared_pointing_at_the_config_layer() {
    let executor = Scripted::new(exited(0, "", ""));
    let response = run_with(&[("test", "bun test")], &executor, RunInput::new("deploy")).await;
    let error = failure(&response);
    assert_eq!(error.kind, ErrorKind::CommandNotDeclared);
    assert_eq!(error.recovery(), Recovery::Abort);
    assert!(error.message.contains(".magistr/workspace.toml"));
    assert!(error.message.contains("[commands]"));
    assert!(error.message.contains("test"));
    assert!(executor.calls().is_empty());
}

#[tokio::test]
async fn a_declared_command_executes_as_one_argv_with_the_extra_args_appended() {
    let executor = Scripted::new(exited(0, "", ""));
    let input = RunInput::new("test").with_args(["--filter", "auth"]);
    let response = run_with(&[("test", "bun test")], &executor, input).await;
    let calls = executor.calls();
    assert_eq!(calls[0].0, vec!["bun", "test", "--filter", "auth"]);
    assert_eq!(calls[0].1.timeout, Duration::from_secs(300));
    assert_eq!(row(&response).command(), "test");
}

#[tokio::test]
async fn schema_misuse_is_refused_before_execution() {
    let executor = Scripted::new(exited(0, "", ""));
    for input in [
        RunInput::new("test").with_timeout_secs(0),
        RunInput::new("test").with_timeout_secs(9999),
        RunInput::new(""),
    ] {
        let response = run_with(&[("test", "bun test")], &executor, input).await;
        assert_eq!(failure(&response).kind, ErrorKind::PatternInvalid);
    }
    assert!(executor.calls().is_empty());
}

#[tokio::test]
async fn munging_shells_and_operators_are_redirected_before_any_execution() {
    let executor = Scripted::new(exited(0, "", ""));
    let munging = run_with(
        &[("find", "grep -rn TODO src")],
        &executor,
        RunInput::new("find"),
    )
    .await;
    let error = failure(&munging);
    assert_eq!(error.kind, ErrorKind::RunRedirected);
    assert_eq!(error.recovery(), Recovery::Abort);
    assert!(error.message.contains("search") && error.message.contains("edit"));

    let shell = run_with(&[("sh", "bash -c ls")], &executor, RunInput::new("sh")).await;
    assert_eq!(failure(&shell).kind, ErrorKind::RunRedirected);
    assert!(failure(&shell).message.contains("shell"));

    let piped = run_with(
        &[("weird", "cargo test | tail -20")],
        &executor,
        RunInput::new("weird"),
    )
    .await;
    assert_eq!(failure(&piped).kind, ErrorKind::RunRedirected);

    let via_args = run_with(
        &[("test", "bun test")],
        &executor,
        RunInput::new("test").with_args(["|", "grep", "fail"]),
    )
    .await;
    assert_eq!(failure(&via_args).kind, ErrorKind::RunRedirected);
    assert!(executor.calls().is_empty(), "rejected before any execution");
}

#[tokio::test]
async fn wrapped_operand_bypasses_never_reach_the_executor() {
    let executor = Scripted::new(exited(0, "", ""));
    for line in [
        "env -u NAME sh -c pwd",
        "nice -n 10 cat Cargo.toml",
        "env NAME=value nice -n 10 sh -c pwd",
    ] {
        let response = run_with(&[("test", line)], &executor, RunInput::new("test")).await;
        assert_eq!(failure(&response).kind, ErrorKind::RunRedirected);
    }
    assert!(executor.calls().is_empty());
}

#[tokio::test]
async fn a_wrapped_command_delivers_exactly_the_admitted_argv() {
    let executor = Scripted::new(exited(0, "", ""));
    let line = "env NAME=value nice cargo test";
    let input = RunInput::new("test").with_args(["--", "has spaces", "$(literal)", "cat"]);
    let (_dir, policy) = workspace(&[("test", line)]);
    let admitted = policy
        .admit_command(&input.command, &input.args)
        .expect("admitted");
    let response = WorkspaceRun::new(&policy, &executor)
        .run(&input, CancellationToken::new())
        .await;
    assert_eq!(row(&response).command(), "test");
    assert_eq!(executor.calls()[0].0, admitted);
}

#[cfg(unix)]
#[tokio::test]
async fn an_admitted_wrapped_process_preserves_literal_arguments() {
    let (dir, policy) = workspace(&[("print", "env MAGISTR_ARGV_TEST=1 /usr/bin/printf")]);
    let executor = super::super::ProcessExecutor::new(dir.path());
    let input = RunInput::new("print").with_args([
        "%s\\n",
        "has spaces",
        "$(touch must-not-exist)",
        "*",
        "$MAGISTR_ARGV_TEST",
    ]);
    let response = WorkspaceRun::new(&policy, &executor)
        .run(&input, CancellationToken::new())
        .await;
    let RunRow::Output {
        exit_code,
        stdout_head,
        ..
    } = row(&response)
    else {
        panic!("expected literal process output");
    };
    assert_eq!(*exit_code, Some(0));
    assert_eq!(
        stdout_head,
        &[
            "has spaces",
            "$(touch must-not-exist)",
            "*",
            "$MAGISTR_ARGV_TEST"
        ]
    );
    assert!(!dir.path().join("must-not-exist").exists());
}

#[tokio::test]
async fn a_missing_program_is_a_typed_failure_not_an_output_row() {
    let executor = Scripted::failing(WorkspaceError::new(
        ErrorKind::FileNotFound,
        "run: the declared program \"protoc\" could not be found",
    ));
    let response = run_with(
        &[("gen", "protoc --version")],
        &executor,
        RunInput::new("gen"),
    )
    .await;
    assert_eq!(failure(&response).kind, ErrorKind::FileNotFound);
}

// ---------------------------------------------------------------------------
// Bundled parsers (retained acceptance #3): realistic fixtures for all six.

const BUN_OUTPUT: &str = "test/auth.test.ts:\n✓ login > accepts a valid token [1.02ms]\n✗ login > rejects an expired token [2.11ms]\n  error: expect(received).toBe(expected)\n  Expected: 401\n  Received: 200\n      at test/auth.test.ts:42:5\n\n 1 pass\n 1 fail";
const JEST_OUTPUT: &str = "FAIL src/auth.test.js\n  ● login › rejects an expired token\n\n    expect(received).toBe(expected) // Object.is equality\n\n    Expected: 401\n    Received: 200\n\n      at Object.<anonymous> (src/auth.test.js:42:19)\n\nTests:       1 failed, 3 passed, 4 total";
const VITEST_OUTPUT: &str = " ❯ src/auth.test.ts > login > rejects an expired token\nAssertionError: expected 200 to be 401\n ❯ src/auth.test.ts:42:19\n\n Tests  1 failed | 3 passed (4)";
const PYTEST_OUTPUT: &str = "=================================== FAILURES ===================================\n____________________________ test_rejects_expired _____________________________\n\n    def test_rejects_expired():\n>       assert resp.status == 401\nE       assert 200 == 401\n\ntests/test_auth.py:42: AssertionError\n=========================== short test summary info ============================\nFAILED tests/test_auth.py::test_rejects_expired - assert 200 == 401\n========================= 1 failed, 3 passed in 0.21s ==========================";
const CARGO_OUTPUT: &str = "running 4 tests\ntest auth::accepts_valid ... ok\ntest auth::rejects_expired ... FAILED\n\nfailures:\n\n---- auth::rejects_expired stdout ----\nthread 'auth::rejects_expired' panicked at src/auth.rs:42:9:\nassertion `left == right` failed\n  left: 200\n right: 401\n\nfailures:\n    auth::rejects_expired\n\ntest result: FAILED. 3 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out";
const GO_OUTPUT: &str = "--- FAIL: TestRejectsExpired (0.00s)\n    auth_test.go:42: got status 200, want 401\nFAIL\nFAIL\texample.com/auth\t0.012s";

#[tokio::test]
async fn each_bundled_runner_extracts_one_failure_with_the_quartet() {
    let cases = [
        (
            "bun",
            "bun test",
            BUN_OUTPUT,
            "login > rejects an expired token",
            "test/auth.test.ts",
            "expect(received).toBe(expected)",
        ),
        (
            "jest",
            "npx jest",
            JEST_OUTPUT,
            "login › rejects an expired token",
            "src/auth.test.js",
            "expect(received).toBe(expected)",
        ),
        (
            "vitest",
            "npx vitest run",
            VITEST_OUTPUT,
            "login > rejects an expired token",
            "src/auth.test.ts",
            "expected 200 to be 401",
        ),
        (
            "pytest",
            "pytest",
            PYTEST_OUTPUT,
            "tests/test_auth.py::test_rejects_expired",
            "tests/test_auth.py",
            "assert 200 == 401",
        ),
        (
            "cargo",
            "cargo test",
            CARGO_OUTPUT,
            "auth::rejects_expired",
            "src/auth.rs",
            "assertion `left == right` failed",
        ),
        (
            "go",
            "go test ./...",
            GO_OUTPUT,
            "TestRejectsExpired",
            "auth_test.go",
            "got status 200, want 401",
        ),
    ];
    for (adapter, line, stdout, test_name, file, assertion) in cases {
        let executor = Scripted::new(exited(1, stdout, ""));
        let response = run_with(&[("t", line)], &executor, RunInput::new("t")).await;
        let (name, failures, summary) = report(row(&response));
        assert_eq!(name, adapter, "{line}");
        assert_eq!(failures.len(), 1, "{line}");
        assert_eq!(failures[0].test_name, test_name, "{line}");
        assert_eq!(failures[0].file.as_deref(), Some(file), "{line}");
        assert!(
            failures[0].assertion.contains(assertion),
            "{line}: {}",
            failures[0].assertion
        );
        assert!(summary.is_some(), "{line}");
    }
}

#[test]
fn every_bundled_runner_claims_only_its_own_command_line() {
    let lines = [
        ("bun", "bun test"),
        ("vitest", "npx vitest run"),
        ("jest", "npx jest"),
        ("pytest", "pytest -q"),
        ("nextest", "cargo nextest run --workspace"),
        ("cargo", "cargo test --workspace"),
        ("go", "go test ./..."),
    ];
    for runner in bundled_runners() {
        for (name, line) in lines {
            assert_eq!(
                runner.can_parse(line),
                runner.name() == name,
                "{} vs {line}",
                runner.name()
            );
        }
    }
    let names: Vec<_> = bundled_runners()
        .iter()
        .map(|runner| runner.name())
        .collect();
    assert!(
        names.iter().position(|name| *name == "nextest")
            < names.iter().position(|name| *name == "cargo")
    );
}

#[tokio::test]
async fn a_green_run_is_a_report_with_zero_failures() {
    let executor = Scripted::new(exited(
        0,
        "running 4 tests\ntest result: ok. 4 passed; 0 failed",
        "",
    ));
    let response = run_with(&[("t", "cargo test")], &executor, RunInput::new("t")).await;
    let (_, failures, summary) = report(row(&response));
    assert!(failures.is_empty());
    assert!(summary.expect("summary").contains("4 passed"));
}

#[tokio::test]
async fn a_claimed_runner_whose_output_defies_extraction_falls_back_honestly() {
    let executor = Scripted::new(exited(101, "Segmentation fault (core dumped)\n", ""));
    let response = run_with(&[("t", "cargo test")], &executor, RunInput::new("t")).await;
    assert!(matches!(row(&response), RunRow::Output { .. }));
}

#[tokio::test]
async fn a_test_run_that_never_compiled_answers_the_compile_errors_as_diagnostics() {
    let executor = Scripted::new(exited(
        101,
        "error[E0308]: mismatched types\n(compile failure)",
        "",
    ));
    let response = run_with(&[("t", "cargo test")], &executor, RunInput::new("t")).await;
    let (_, diagnostics, summary) = diagnostic_row(row(&response));
    assert_eq!(diagnostics[0].code.as_deref(), Some("E0308"));
    assert_eq!(diagnostics[0].message, "mismatched types");
    assert_eq!(summary, "1 error");
}

// ---------------------------------------------------------------------------
// Head+tail fallback with a truncation manifest

#[tokio::test]
async fn an_unknown_commands_long_output_is_cut_with_a_manifest_never_dumped() {
    let lines: Vec<String> = (0..500).map(|index| format!("line {index}")).collect();
    let executor = Scripted::new(exited(0, &lines.join("\n"), ""));
    let response = run_with(
        &[("gen", "protoc --version")],
        &executor,
        RunInput::new("gen").with_budget(100),
    )
    .await;
    let RunRow::Output {
        stdout_head,
        stdout_tail,
        ..
    } = row(&response)
    else {
        panic!("expected an output row");
    };
    assert_eq!(stdout_head[0], "line 0");
    assert_eq!(stdout_tail.last().map(String::as_str), Some("line 499"));
    let (dropped, cursor) = truncation(&response).expect("a manifest");
    assert!(dropped > 0);
    assert_eq!(cursor, format!("line:{}", stdout_head.len()));
    if let WorkspaceResponse::Success { results, .. } = &response {
        assert_eq!(
            results.truncation().map(|cut| cut.reason()),
            Some(TruncationReason::Budget)
        );
    }
}

#[tokio::test]
async fn short_output_arrives_whole_and_a_manifest_only_exists_when_something_was_cut() {
    let executor = Scripted::new(exited(0, "ok\n", ""));
    let response = run_with(
        &[("gen", "protoc --version")],
        &executor,
        RunInput::new("gen"),
    )
    .await;
    assert!(truncation(&response).is_none());
    assert!(matches!(row(&response), RunRow::Output { .. }));
}

#[tokio::test]
async fn a_timeout_is_data_with_partial_output_and_a_note_not_an_error() {
    let executor = Scripted::new(ok(RunExecution {
        exit_code: None,
        stdout: "partial".into(),
        timed_out: true,
        duration_ms: 2000,
        ..RunExecution::default()
    }));
    let response = run_with(
        &[("slow", "protoc --wait")],
        &executor,
        RunInput::new("slow").with_timeout_secs(2),
    )
    .await;
    let output = row(&response);
    assert!(output.timed_out());
    assert_eq!(output.exit_code(), None);
    assert!(
        notes(&response)
            .iter()
            .any(|note| note.contains("timed out"))
    );
}

#[tokio::test]
async fn cancellation_and_dropped_bytes_are_named_in_notes() {
    let executor = Scripted::new(ok(RunExecution {
        exit_code: None,
        cancelled: true,
        dropped_bytes: 4096,
        ..RunExecution::default()
    }));
    let response = run_with(&[("t", "cargo test")], &executor, RunInput::new("t")).await;
    assert!(matches!(row(&response), RunRow::Output { .. }));
    let notes = notes(&response);
    assert!(notes.iter().any(|note| note.contains("cancelled")));
    assert!(notes.iter().any(|note| note.contains("4096 bytes")));
}

// ---------------------------------------------------------------------------
// Plugin runner lanes: consulted first, degrade honestly.

#[tokio::test]
async fn plugin_lanes_are_consulted_before_the_bundled_parsers_and_degrade_honestly() {
    let (_dir, policy) = workspace(&[("t", "cargo test")]);
    let executor = Scripted::new(exited(101, CARGO_OUTPUT, ""));
    let claimed = FakeLane {
        name: "acme.runner",
        answer: Ok(Some(RunReport {
            failures: vec![RunFailure {
                test_name: "from plugin".into(),
                file: None,
                assertion: "x".into(),
                trace: String::new(),
            }],
            summary: None,
        })),
    };
    let unclaimed = FakeLane {
        name: "acme.other",
        answer: Err(WorkspaceError::new(
            ErrorKind::CommandNotDeclared,
            "did not claim",
        )),
    };
    let crashed = FakeLane {
        name: "acme.crash",
        answer: Err(WorkspaceError::new(
            ErrorKind::PluginCrashed {
                plugin: "acme.crash".into(),
            },
            "guest trap",
        )),
    };
    let claimed_without_structure = FakeLane {
        name: "acme.none",
        answer: Ok(None),
    };
    let input = RunInput::new("t");

    let response = WorkspaceRun::new(&policy, &executor)
        .with_plugin_lanes(vec![&unclaimed, &claimed])
        .run(&input, CancellationToken::new())
        .await;
    let (adapter, failures, _) = report(row(&response));
    assert_eq!(adapter, "acme.runner");
    assert_eq!(failures[0].test_name, "from plugin");

    let response = WorkspaceRun::new(&policy, &executor)
        .with_plugin_lanes(vec![&crashed, &unclaimed])
        .run(&input, CancellationToken::new())
        .await;
    let (adapter, _, _) = report(row(&response));
    assert_eq!(
        adapter, "cargo",
        "a dead lane degrades to the bundled parser"
    );
    assert!(
        notes(&response)
            .iter()
            .any(|note| note.contains("acme.crash"))
    );

    let response = WorkspaceRun::new(&policy, &executor)
        .with_plugin_lanes(vec![&claimed_without_structure])
        .run(&input, CancellationToken::new())
        .await;
    assert!(
        matches!(row(&response), RunRow::Output { .. }),
        "a claimant without structure ends the search: the honest cut, not a weaker guess"
    );
}

// ---------------------------------------------------------------------------
// The cargo family over the retained goldens (specs/013)

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../subsystems/workspace/reference/test/support/golden/cargo")
}

/// A captured input: `<name>.txt` (stdout, or the 2>&1 capture) plus an
/// optional `<name>.stderr.txt`.
fn captured(name: &str) -> (String, String) {
    let stdout =
        std::fs::read_to_string(golden_dir().join(format!("{name}.txt"))).expect("golden stdout");
    let stderr = std::fs::read_to_string(golden_dir().join(format!("{name}.stderr.txt")))
        .unwrap_or_default();
    (stdout, stderr)
}

fn golden(name: &str, actual: &RunRow) {
    let expected: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(golden_dir().join(format!("{name}.expected.json")))
            .expect("golden"),
    )
    .expect("golden JSON");
    assert_eq!(
        serde_json::to_value(actual).expect("row JSON"),
        expected,
        "{name}"
    );
}

async fn golden_run(
    name: &str,
    line: &str,
    exit_code: i32,
    budget: Option<u64>,
) -> WorkspaceResponse<RunRow> {
    let (stdout, stderr) = captured(name);
    let executor = Scripted::new(exited(exit_code, &stdout, &stderr));
    let mut input = RunInput::new("c");
    if let Some(budget) = budget {
        input = input.with_budget(budget);
    }
    run_with(&[("c", line)], &executor, input).await
}

fn by_name<'f>(failures: &'f [RunFailure], test_name: &str) -> &'f RunFailure {
    failures
        .iter()
        .find(|failure| failure.test_name == test_name)
        .unwrap_or_else(|| panic!("no failure named {test_name}"))
}

#[tokio::test]
async fn colored_cargo_reports_preserve_failures_and_diagnostics() {
    for (name, command) in [
        (
            "cargo-test-workspace",
            "cargo test --workspace --no-fail-fast",
        ),
        ("build-human-warn-error", "cargo build --workspace"),
    ] {
        let (stdout, stderr) = captured(name);
        let decorated = |text: &str| {
            text.lines()
                .map(|line| {
                    format!("\x1b[1;32m\x1b]8;;file:///fixture\x1b\\{line}\x1b]8;;\x1b\\\x1b[0m\n")
                })
                .collect::<String>()
        };
        let executor = Scripted::new(exited(101, &decorated(&stdout), &decorated(&stderr)));
        let response = run_with(&[("c", command)], &executor, RunInput::new("c")).await;
        golden(name, row(&response));
    }
}

#[tokio::test]
async fn unstructured_output_keeps_the_original_capture() {
    let raw = "\x1b[31mred\x1b[0m";
    let executor = Scripted::new(exited(0, raw, ""));
    let response = run_with(&[("c", "printf red")], &executor, RunInput::new("c")).await;
    let RunRow::Output { stdout_head, .. } = row(&response) else {
        panic!("unclaimed command keeps raw output");
    };
    assert_eq!(stdout_head, &[raw]);
}

#[tokio::test]
async fn cargo_test_one_binary_matches_the_golden() {
    let response = golden_run("cargo-test-single", "cargo test", 101, None).await;
    let (adapter, failures, summary) = report(row(&response));
    assert_eq!(adapter, "cargo");
    let mut names: Vec<_> = failures
        .iter()
        .map(|failure| failure.test_name.as_str())
        .collect();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "tests::fails_assert",
            "tests::fails_eq",
            "tests::fails_panic"
        ]
    );
    let plain = by_name(failures, "tests::fails_panic");
    assert_eq!(plain.assertion, "boom 42");
    assert_eq!(plain.file.as_deref(), Some("crates/alpha/src/lib.rs"));
    assert_eq!(
        plain.trace.lines().next(),
        Some("crates/alpha/src/lib.rs:11:24")
    );
    let eq = by_name(failures, "tests::fails_eq");
    assert_eq!(
        eq.assertion,
        "assertion `left == right` failed: math is hard"
    );
    assert!(eq.trace.contains("left: 3") && eq.trace.contains("right: 4"));
    assert!(!eq.trace.contains("RUST_BACKTRACE"));
    assert_eq!(
        by_name(failures, "tests::fails_assert").assertion,
        "assertion failed: add(1, 1) == 3"
    );
    assert_eq!(
        summary,
        Some(
            "test result: FAILED. 1 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s"
        )
    );
    golden("cargo-test-single", row(&response));
}

#[tokio::test]
async fn cargo_test_workspace_run_attributes_every_binary_and_aggregates_the_summary() {
    let response = golden_run(
        "cargo-test-workspace",
        "cargo test --workspace --no-fail-fast",
        101,
        None,
    )
    .await;
    let (_, failures, summary) = report(row(&response));
    let names: Vec<_> = failures
        .iter()
        .map(|failure| failure.test_name.as_str())
        .collect();
    assert_eq!(
        names,
        [
            "alpha tests::fails_eq",
            "alpha tests::fails_assert",
            "alpha tests::fails_panic",
            "integration integration_fails"
        ]
    );
    let oob = by_name(failures, "integration integration_fails");
    assert_eq!(
        oob.file.as_deref(),
        Some("crates/beta/tests/integration.rs")
    );
    assert_eq!(
        oob.assertion,
        "index out of bounds: the len is 0 but the index is 0"
    );
    assert_eq!(
        summary,
        Some(
            "2 passed; 4 failed; 0 ignored across 5 binaries — rerun: -p alpha --lib, -p beta --test integration"
        )
    );
    golden("cargo-test-workspace", row(&response));
}

#[tokio::test]
async fn cargo_test_backtrace_keeps_workspace_frames_and_drops_toolchain_frames() {
    let response = golden_run(
        "cargo-test-backtrace",
        "cargo test -p alpha --lib tests::fails_panic",
        101,
        None,
    )
    .await;
    let (_, failures, _) = report(row(&response));
    assert_eq!(failures.len(), 1);
    let lines: Vec<_> = failures[0].trace.lines().collect();
    assert_eq!(lines[0], "crates/alpha/src/lib.rs:11:24");
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("alpha::tests::fails_panic at ./src/lib.rs:11"))
    );
    assert!(!lines.iter().any(|line| line.contains("/rustc/")
        || line.contains("panicking.rs")
        || line.contains(".rustup")));
    assert!(lines.len() <= 8);
    golden("cargo-test-backtrace", row(&response));
}

#[tokio::test]
async fn nextest_workspace_matches_the_golden() {
    let response = golden_run(
        "nextest-workspace",
        "cargo nextest run --workspace",
        100,
        None,
    )
    .await;
    let (adapter, failures, summary) = report(row(&response));
    assert_eq!(adapter, "nextest");
    let names: Vec<_> = failures
        .iter()
        .map(|failure| failure.test_name.as_str())
        .collect();
    assert_eq!(
        names,
        [
            "alpha tests::fails_eq",
            "beta::integration integration_fails",
            "alpha tests::fails_panic",
            "alpha tests::fails_assert"
        ]
    );
    let oob = by_name(failures, "beta::integration integration_fails");
    assert_eq!(
        oob.file.as_deref(),
        Some("crates/beta/tests/integration.rs")
    );
    assert_eq!(
        oob.trace.lines().next(),
        Some("crates/beta/tests/integration.rs:2:61")
    );
    assert_eq!(
        by_name(failures, "alpha tests::fails_panic").assertion,
        "boom 42"
    );
    assert_eq!(
        summary,
        Some(
            "Summary [ 0.024s] 6 tests run: 2 passed, 4 failed, 0 skipped (cancelled: 5 still running)"
        )
    );
    golden("nextest-workspace", row(&response));
}

#[tokio::test]
async fn nextest_statuses_without_a_panic_name_the_status_and_retries_are_not_failures() {
    let stderr = [
        "    Starting 3 tests across 1 binary",
        "        TRY 1 FAIL [   0.010s] (1/3) alpha tests::flaky",
        "      FLAKY 2/3 [   0.011s] (1/3) alpha tests::flaky",
        "     TIMEOUT [  60.000s] (2/3) alpha tests::hangs",
        "  stdout ───",
        "",
        "    running 1 test",
        "",
        "        PASS [   0.001s] (3/3) alpha tests::ok",
        "     Summary [  60.020s] 3 tests run: 2 passed (1 flaky), 1 failed, 0 skipped",
        "     TIMEOUT [  60.000s] (2/3) alpha tests::hangs",
    ]
    .join("\n");
    let executor = Scripted::new(exited(100, "", &stderr));
    let response = run_with(&[("c", "cargo nextest run")], &executor, RunInput::new("c")).await;
    let (_, failures, summary) = report(row(&response));
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].test_name, "alpha tests::hangs");
    assert_eq!(failures[0].assertion, "TIMEOUT");
    assert_eq!(
        summary,
        Some("Summary [ 60.020s] 3 tests run: 2 passed (1 flaky), 1 failed, 0 skipped")
    );
}

#[tokio::test]
async fn rustc_json_diagnostics_match_the_golden_and_equal_the_rendered_form() {
    let json = golden_run(
        "build-json-warn-error",
        "cargo build --workspace --message-format=json",
        101,
        None,
    )
    .await;
    let (extractor, diagnostics, summary) = diagnostic_row(row(&json));
    assert_eq!(extractor, "cargo");
    let levels: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.level)
        .collect();
    assert_eq!(
        levels,
        [
            DiagnosticLevel::Error,
            DiagnosticLevel::Error,
            DiagnosticLevel::Warning
        ]
    );
    let first = &diagnostics[0];
    assert_eq!(first.code.as_deref(), Some("E0308"));
    assert_eq!(first.message, "mismatched types");
    assert_eq!(first.file.as_deref(), Some("crates/gamma/src/lib.rs"));
    assert_eq!((first.line, first.column), (Some(1), Some(42)));
    assert!(first.detail.contains("help: try using a conversion method"));
    assert!(
        !first
            .detail
            .lines()
            .next()
            .unwrap_or_default()
            .contains("-->")
    );
    assert_eq!(diagnostics[2].code.as_deref(), Some("unused_variables"));
    assert_eq!(diagnostics[2].message, "unused variable: `y`");
    assert_eq!(diagnostics[2].line, Some(2));
    assert_eq!(summary, "2 errors, 1 warning");
    golden("build-json-warn-error", row(&json));

    let human = golden_run(
        "build-human-warn-error",
        "cargo build --workspace",
        101,
        None,
    )
    .await;
    let (_, human_diagnostics, human_summary) = diagnostic_row(row(&human));
    assert_eq!(human_diagnostics, diagnostics);
    assert_eq!(human_summary, summary);
    golden("build-human-warn-error", row(&human));
}

#[tokio::test]
async fn clippy_json_and_rendered_are_equal_with_the_lint_name_as_the_code() {
    let json = golden_run(
        "clippy-json",
        "cargo clippy -p alpha --message-format=json",
        0,
        None,
    )
    .await;
    let human = golden_run("clippy-human", "cargo clippy -p alpha", 0, None).await;
    let (_, json_diagnostics, _) = diagnostic_row(row(&json));
    let (_, human_diagnostics, _) = diagnostic_row(row(&human));
    assert_eq!(json_diagnostics.len(), 1);
    assert_eq!(
        json_diagnostics[0].code.as_deref(),
        Some("unused_variables")
    );
    assert_eq!(human_diagnostics, json_diagnostics);
    golden("clippy-json", row(&json));
    golden("clippy-human", row(&human));
}

#[tokio::test]
async fn a_green_build_is_an_empty_diagnostics_row_not_head_tail() {
    let response = golden_run("build-green", "cargo build", 0, None).await;
    let (_, diagnostics, summary) = diagnostic_row(row(&response));
    assert!(diagnostics.is_empty());
    assert_eq!(summary, "no diagnostics");
    golden("build-green", row(&response));
}

#[tokio::test]
async fn cargo_test_claims_go_to_diagnostics_only_when_the_build_failed() {
    let failed = golden_run("build-human-warn-error", "cargo test", 101, None).await;
    let (_, diagnostics, _) = diagnostic_row(row(&failed));
    assert_eq!(diagnostics[0].code.as_deref(), Some("E0308"));

    let compiled = golden_run("cargo-test-single", "cargo test", 101, None).await;
    assert!(matches!(row(&compiled), RunRow::Report { .. }));
}

#[tokio::test]
async fn unrecognizable_failures_fall_back_and_location_less_errors_are_still_rows() {
    let executor = Scripted::new(exited(101, "", "Segmentation fault (core dumped)\n"));
    let response = run_with(&[("c", "cargo build")], &executor, RunInput::new("c")).await;
    let RunRow::Output { stderr_head, .. } = row(&response) else {
        panic!("expected head+tail");
    };
    assert!(stderr_head[0].contains("Segmentation fault"));

    let stderr = "error: linking with `cc` failed: exit status: 1\n  |\n  = note: some arguments are omitted\n\nerror: could not compile `x` (bin) due to 1 previous error";
    let executor = Scripted::new(exited(101, "", stderr));
    let response = run_with(&[("c", "cargo build")], &executor, RunInput::new("c")).await;
    let (_, diagnostics, summary) = diagnostic_row(row(&response));
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].message,
        "linking with `cc` failed: exit status: 1"
    );
    assert_eq!(diagnostics[0].file, None);
    assert_eq!(summary, "1 error");

    let executor = Scripted::new(exited(101, "", "error: no such command: `clippy`\n"));
    let response = run_with(&[("c", "cargo clippy")], &executor, RunInput::new("c")).await;
    let (_, missing, _) = diagnostic_row(row(&response));
    assert_eq!(missing[0].message, "no such command: `clippy`");
}

#[tokio::test]
async fn over_budget_keeps_errors_before_warnings_with_a_manifest_and_a_whole_run_summary() {
    let line = "cargo build --workspace --message-format=json";
    let full = golden_run("build-json-warn-error", line, 101, None).await;
    let RunRow::Diagnostics {
        diagnostics: all, ..
    } = row(&full).clone()
    else {
        panic!("expected diagnostics");
    };
    let mut two_errors = row(&full).clone();
    if let RunRow::Diagnostics { diagnostics, .. } = &mut two_errors {
        diagnostics.truncate(2);
    }
    let wire = serde_json::to_string(&two_errors).expect("row JSON");
    let budget = (wire.encode_utf16().count() as u64).div_ceil(4) + 1;
    let response = golden_run("build-json-warn-error", line, 101, Some(budget)).await;
    let (_, kept, summary) = diagnostic_row(row(&response));
    let levels: Vec<_> = kept.iter().map(|diagnostic| diagnostic.level).collect();
    assert_eq!(levels, [DiagnosticLevel::Error, DiagnosticLevel::Error]);
    assert_eq!(summary, "2 errors, 1 warning");
    assert_eq!(truncation(&response), Some((1, "diagnostic:2".to_owned())));
    if let WorkspaceResponse::Success {
        budget: used,
        results,
        ..
    } = &response
    {
        assert!(used.used <= budget);
        assert!(
            results
                .truncation()
                .expect("cut")
                .narrow()
                .iter()
                .any(|hint| hint.contains("warnings were dropped first"))
        );
    }
    assert_eq!(all.len(), 3);

    // Even a budget too small for one diagnostic keeps one — never an empty
    // row pretending there was nothing to say.
    let tiny = golden_run("build-json-warn-error", line, 101, Some(1)).await;
    let (_, kept, _) = diagnostic_row(row(&tiny));
    assert_eq!(kept.len(), 1);
    assert_eq!(truncation(&tiny).map(|cut| cut.0), Some(2));
}

#[test]
fn the_same_warning_from_a_lib_and_its_test_build_collapses_to_one() {
    let one = std::fs::read_to_string(golden_dir().join("clippy-json.txt")).expect("golden");
    let out = extract_cargo_diagnostics(&RunOutput {
        command: "cargo clippy --all-targets --message-format=json".into(),
        exit_code: 0,
        stdout: format!("{one}\n{one}"),
        stderr: String::new(),
    });
    assert_eq!(out.map(|diagnostics| diagnostics.len()), Some(1));
}

#[test]
fn cargo_subcommand_skips_toolchain_overrides_and_flags() {
    assert_eq!(
        cargo_subcommand("cargo +nightly build").as_deref(),
        Some("build")
    );
    assert_eq!(
        cargo_subcommand("cargo --locked test -p x").as_deref(),
        Some("test")
    );
    assert_eq!(
        cargo_subcommand("cargo nextest run").as_deref(),
        Some("nextest")
    );
    assert_eq!(cargo_subcommand("bun test"), None);
}
