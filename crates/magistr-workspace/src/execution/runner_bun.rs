use std::sync::LazyLock;

use regex::Regex;

use super::{RunFailure, RunOutput, RunReport, RunnerParser, run_parse};

static FAIL: LazyLock<Regex> =
    LazyLock::new(|| run_parse::regex(r"^\s*(?:✗|\(fail\))\s+(.+?)(?:\s+\[[\d.]+m?s\])?\s*$"));
static BOUNDARY: LazyLock<Regex> =
    LazyLock::new(|| run_parse::regex(r"^\s*(?:✗|✓|\(fail\)|\(pass\))\s"));
static ERR: LazyLock<Regex> =
    LazyLock::new(|| run_parse::regex(r"(?i)^\s*(?:error|AssertionError)[:\s](.*)$"));
static AT: LazyLock<Regex> =
    LazyLock::new(|| run_parse::regex(r"(?-u:\b)at\s+(?:.*\()?([^\s()]+?):(\d+)(?::\d+)?\)?\s*$"));
static SUMMARY: LazyLock<Regex> =
    LazyLock::new(|| run_parse::regex(r"(?i)^\s*(\d+\s+(?:pass|fail)(?:ed)?)\s*$"));

/// `bun test`: `✗ name [1.02ms]` failures, the first error line as the
/// assertion, `at file:line` frames; " N pass" and " N fail" joined.
pub struct BunRunner;

impl RunnerParser for BunRunner {
    fn name(&self) -> &'static str {
        "bun"
    }

    fn can_parse(&self, command_line: &str) -> bool {
        run_parse::has_token(command_line, "bun") && run_parse::has_token(command_line, "test")
    }

    fn parse_output(&self, output: &RunOutput) -> Option<RunReport> {
        let lines = output.combined_lines();
        let mut failures = Vec::new();
        let mut summary: Option<String> = None;
        for (index, line) in lines.iter().enumerate() {
            if let Some(fail) = FAIL.captures(line) {
                let mut assertion = String::new();
                let mut trace = Vec::new();
                let mut file = None;
                for body in lines.iter().skip(index + 1).take(29) {
                    if BOUNDARY.is_match(body) {
                        break;
                    }
                    if let Some(err) = ERR.captures(body)
                        && assertion.is_empty()
                    {
                        assertion = err[1].trim().to_owned();
                    }
                    if let Some(at) = AT.captures(body) {
                        file.get_or_insert_with(|| at[1].to_owned());
                        trace.push(body.trim().to_owned());
                    }
                }
                failures.push(RunFailure {
                    test_name: fail[1].trim().to_owned(),
                    file,
                    assertion,
                    trace: run_parse::trim_trace(&trace),
                });
            }
            if let Some(sum) = SUMMARY.captures(line) {
                summary = Some(match summary {
                    None => sum[1].to_owned(),
                    Some(previous) => format!("{previous}, {}", &sum[1]),
                });
            }
        }
        if failures.is_empty() && output.exit_code != 0 {
            return None;
        }
        Some(RunReport { failures, summary })
    }
}
