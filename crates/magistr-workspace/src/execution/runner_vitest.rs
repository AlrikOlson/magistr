use std::sync::LazyLock;

use regex::Regex;

use super::{RunFailure, RunOutput, RunReport, RunnerParser, run_parse};

static FAIL: LazyLock<Regex> = LazyLock::new(|| {
    run_parse::regex(r"^\s*(?:❯|×|FAIL)\s+(\S+\.[cm]?[jt]sx?)\s*>\s*(.+?)(?:\s+\d+ms)?\s*$")
});
static BOUNDARY: LazyLock<Regex> =
    LazyLock::new(|| run_parse::regex(r"^\s*(?:❯|×|FAIL)\s+\S+\s*>"));
static ERR: LazyLock<Regex> =
    LazyLock::new(|| run_parse::regex(r"^\s*(?:AssertionError|Error|TypeError)[:\s](.*)$"));
static AT: LazyLock<Regex> =
    LazyLock::new(|| run_parse::regex(r"^\s*❯\s+.*?([^\s()]+):(\d+)(?::\d+)?\s*$"));
static SUMMARY_FAILED: LazyLock<Regex> =
    LazyLock::new(|| run_parse::regex(r"^\s*Tests\s+.*(?-u:\b)failed(?-u:\b)"));
static SUMMARY_PASSED: LazyLock<Regex> =
    LazyLock::new(|| run_parse::regex(r"^\s*Tests\s+\d+\s+passed"));

/// vitest: `❯ file > name` failures with the file in the header line, an
/// `AssertionError:` line as the assertion and `❯ file:line` frames.
pub struct VitestRunner;

impl RunnerParser for VitestRunner {
    fn name(&self) -> &'static str {
        "vitest"
    }

    fn can_parse(&self, command_line: &str) -> bool {
        run_parse::has_token(command_line, "vitest")
    }

    fn parse_output(&self, output: &RunOutput) -> Option<RunReport> {
        let lines = output.combined_lines();
        let mut failures = Vec::new();
        let mut summary = None;
        for (index, line) in lines.iter().enumerate() {
            if let Some(fail) = FAIL.captures(line) {
                let mut assertion = String::new();
                let mut trace = Vec::new();
                for body in lines.iter().skip(index + 1).take(39) {
                    if BOUNDARY.is_match(body) {
                        break;
                    }
                    if let Some(err) = ERR.captures(body)
                        && assertion.is_empty()
                    {
                        assertion = err[1].trim().to_owned();
                    }
                    if AT.is_match(body) {
                        trace.push(body.trim().to_owned());
                    }
                }
                failures.push(RunFailure {
                    test_name: fail[2].trim().to_owned(),
                    file: Some(fail[1].to_owned()),
                    assertion,
                    trace: run_parse::trim_trace(&trace),
                });
            }
            if SUMMARY_FAILED.is_match(line) || SUMMARY_PASSED.is_match(line) {
                summary = Some(run_parse::collapse_whitespace(line.trim()));
            }
        }
        if failures.is_empty() && output.exit_code != 0 {
            return None;
        }
        Some(RunReport { failures, summary })
    }
}
