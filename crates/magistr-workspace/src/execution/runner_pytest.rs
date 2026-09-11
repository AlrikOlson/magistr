use std::{collections::HashMap, sync::LazyLock};

use regex::Regex;

use super::{RunFailure, RunOutput, RunReport, RunnerParser, run_parse};

static MODULE_INVOCATION: LazyLock<Regex> =
    LazyLock::new(|| run_parse::regex(r"\s-m\s+pytest(?-u:\b)"));
static HEADER: LazyLock<Regex> = LazyLock::new(|| run_parse::regex(r"^_{3,}\s+(.+?)\s+_{3,}$"));
static BLOCK_END: LazyLock<Regex> = LazyLock::new(|| run_parse::regex(r"^(?:_{3,}|={3,})"));
static ERROR_LINE: LazyLock<Regex> = LazyLock::new(|| run_parse::regex(r"^E\s"));
static LOCATION_LINE: LazyLock<Regex> = LazyLock::new(|| run_parse::regex(r"^\S+\.py:\d+"));
static FAILED: LazyLock<Regex> =
    LazyLock::new(|| run_parse::regex(r"^FAILED\s+(\S+?)::(\S+?)(?:\s+-\s+(.*))?$"));
static SUMMARY: LazyLock<Regex> = LazyLock::new(|| {
    run_parse::regex(r"^=+\s+(.*(?-u:\b)failed(?-u:\b).*|.*(?-u:\b)passed(?-u:\b).*)\s+=+$")
});

/// pytest: long-form `___ test ___` blocks supply `E` lines and `.py:N`
/// locations as the trace; the short summary's `FAILED file::test - msg`
/// lines are the failures, matched to their blocks by (parametrized) name.
pub struct PytestRunner;

impl RunnerParser for PytestRunner {
    fn name(&self) -> &'static str {
        "pytest"
    }

    fn can_parse(&self, command_line: &str) -> bool {
        run_parse::has_token(command_line, "pytest")
            || run_parse::has_token(command_line, "py.test")
            || (run_parse::tokens_of(command_line)
                .iter()
                .any(|token| token.starts_with("python"))
                && MODULE_INVOCATION.is_match(command_line))
    }

    fn parse_output(&self, output: &RunOutput) -> Option<RunReport> {
        let lines = output.combined_lines();
        let mut traces: HashMap<String, String> = HashMap::new();
        for (index, line) in lines.iter().enumerate() {
            let Some(header) = HEADER.captures(line) else {
                continue;
            };
            let mut block = Vec::new();
            for body in lines.iter().skip(index + 1) {
                if BLOCK_END.is_match(body) {
                    break;
                }
                if ERROR_LINE.is_match(body) || LOCATION_LINE.is_match(body) {
                    block.push(body.trim().to_owned());
                }
            }
            traces.insert(header[1].trim().to_owned(), run_parse::trim_trace(&block));
        }
        let mut failures = Vec::new();
        let mut summary = None;
        for line in &lines {
            if let Some(failed) = FAILED.captures(line) {
                let file = failed[1].to_owned();
                let test_name = failed[2].to_owned();
                let short_name = test_name.split('[').next().unwrap_or(&test_name).to_owned();
                failures.push(RunFailure {
                    test_name: format!("{file}::{test_name}"),
                    file: Some(file),
                    assertion: failed
                        .get(3)
                        .map(|assertion| assertion.as_str().trim().to_owned())
                        .unwrap_or_default(),
                    trace: traces
                        .get(&test_name)
                        .or_else(|| traces.get(&short_name))
                        .cloned()
                        .unwrap_or_default(),
                });
            }
            if let Some(sum) = SUMMARY.captures(line) {
                summary = Some(sum[1].trim().to_owned());
            }
        }
        if failures.is_empty() && output.exit_code != 0 {
            return None;
        }
        Some(RunReport { failures, summary })
    }
}
