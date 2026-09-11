use std::sync::LazyLock;

use regex::Regex;

use super::{RunFailure, RunOutput, RunReport, RunnerParser, run_parse};

static FAIL: LazyLock<Regex> =
    LazyLock::new(|| run_parse::regex(r"^(?:\s*)--- FAIL:\s+(\S+)\s+\(([\d.]+s)\)"));
static BOUNDARY: LazyLock<Regex> =
    LazyLock::new(|| run_parse::regex(r"^(?:\s*)--- (?:FAIL|PASS):|^(?:FAIL|PASS|ok)(?-u:\b)"));
static AT: LazyLock<Regex> =
    LazyLock::new(|| run_parse::regex(r"^\s+([^\s:]+\.go):(\d+):\s*(.*)$"));
static SUMMARY: LazyLock<Regex> =
    LazyLock::new(|| run_parse::regex(r"^(?:FAIL|ok)\s+\S+\s+[\d.]+s"));

/// `go test`: `--- FAIL: TestName (0.00s)` with indented `file.go:N: msg`
/// lines; the package `FAIL`/`ok` line is the summary.
pub struct GoRunner;

impl RunnerParser for GoRunner {
    fn name(&self) -> &'static str {
        "go"
    }

    fn can_parse(&self, command_line: &str) -> bool {
        run_parse::has_token(command_line, "go") && run_parse::has_token(command_line, "test")
    }

    fn parse_output(&self, output: &RunOutput) -> Option<RunReport> {
        let lines = output.combined_lines();
        let mut failures = Vec::new();
        let mut summary = None;
        for (index, line) in lines.iter().enumerate() {
            if let Some(fail) = FAIL.captures(line) {
                let mut assertion = String::new();
                let mut file = None;
                let mut trace = Vec::new();
                for body in lines.iter().skip(index + 1).take(29) {
                    if BOUNDARY.is_match(body) {
                        break;
                    }
                    if let Some(at) = AT.captures(body) {
                        file.get_or_insert_with(|| at[1].to_owned());
                        if assertion.is_empty() {
                            assertion = at[3].trim().to_owned();
                        }
                        trace.push(body.trim().to_owned());
                    }
                }
                failures.push(RunFailure {
                    test_name: fail[1].to_owned(),
                    file,
                    assertion,
                    trace: run_parse::trim_trace(&trace),
                });
            }
            if SUMMARY.is_match(line) {
                summary = Some(line.trim().to_owned());
            }
        }
        if failures.is_empty() && output.exit_code != 0 {
            return None;
        }
        Some(RunReport { failures, summary })
    }
}
