use std::sync::LazyLock;

use regex::Regex;

use super::{RunFailure, RunOutput, RunReport, RunnerParser, run_parse};

static FAIL_FILE: LazyLock<Regex> = LazyLock::new(|| run_parse::regex(r"^\s*FAIL\s+(\S+)"));
static BLOCK: LazyLock<Regex> = LazyLock::new(|| run_parse::regex(r"^\s*●\s+(.+)$"));
static SUITE_FAILED: LazyLock<Regex> = LazyLock::new(|| run_parse::regex(r"(?i)Test suite failed"));
static BOUNDARY: LazyLock<Regex> = LazyLock::new(|| run_parse::regex(r"^\s*●\s"));
static NON_BLANK: LazyLock<Regex> = LazyLock::new(|| run_parse::regex(r"\S"));
static AT: LazyLock<Regex> =
    LazyLock::new(|| run_parse::regex(r"^\s*at\s+.*?\(?([^\s():]+):(\d+)(?::\d+)?\)?\s*$"));
static SUMMARY_FAILED: LazyLock<Regex> =
    LazyLock::new(|| run_parse::regex(r"^\s*Tests:\s+.*(?-u:\b)failed(?-u:\b)"));
static SUMMARY_PASSED: LazyLock<Regex> =
    LazyLock::new(|| run_parse::regex(r"^\s*Tests:\s+\d+\s+passed"));

/// jest: `FAIL <file>` then `● suite › name` blocks; the first non-blank
/// line is the assertion, `at (file:line:col)` frames the trace.
pub struct JestRunner;

impl RunnerParser for JestRunner {
    fn name(&self) -> &'static str {
        "jest"
    }

    fn can_parse(&self, command_line: &str) -> bool {
        run_parse::has_token(command_line, "jest")
    }

    fn parse_output(&self, output: &RunOutput) -> Option<RunReport> {
        let lines = output.combined_lines();
        let mut failures = Vec::new();
        let mut summary = None;
        let mut last_failed_file: Option<String> = None;
        for (index, line) in lines.iter().enumerate() {
            if let Some(fail_file) = FAIL_FILE.captures(line) {
                last_failed_file = Some(fail_file[1].to_owned());
            }
            if let Some(block) = BLOCK.captures(line)
                && !SUITE_FAILED.is_match(&block[1])
            {
                let mut assertion = String::new();
                let mut trace = Vec::new();
                let mut file: Option<String> = None;
                for body in lines.iter().skip(index + 1).take(39) {
                    if BOUNDARY.is_match(body) {
                        break;
                    }
                    if assertion.is_empty() && NON_BLANK.is_match(body) {
                        assertion = body.trim().to_owned();
                    }
                    if let Some(at) = AT.captures(body) {
                        file.get_or_insert_with(|| at[1].to_owned());
                        trace.push(body.trim().to_owned());
                    }
                }
                failures.push(RunFailure {
                    test_name: block[1].trim().to_owned(),
                    file: file.or_else(|| last_failed_file.clone()),
                    assertion,
                    trace: run_parse::trim_trace(&trace),
                });
            }
            if SUMMARY_FAILED.is_match(line) || SUMMARY_PASSED.is_match(line) {
                summary = Some(line.trim().to_owned());
            }
        }
        if failures.is_empty() && output.exit_code != 0 {
            return None;
        }
        Some(RunReport { failures, summary })
    }
}
