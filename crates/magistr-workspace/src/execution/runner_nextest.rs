use std::{collections::HashSet, sync::LazyLock};

use regex::Regex;

use super::{
    RunFailure, RunOutput, RunReport, RunnerParser, cargo_panic::parse_panic_block, run_parse,
};

/// A test's final status line: `FAIL [ 0.008s] (1/6) alpha tests::x`.
/// `TRY n` lines are retries; the final status follows and is the one kept.
static STATUS: LazyLock<Regex> = LazyLock::new(|| {
    run_parse::regex(
        r"^\s*(?:(TRY \d+) )?(PASS|FAIL|SLOW|LEAK|FLAKY|TIMEOUT|SIGABRT|SIGSEGV|SIGKILL|SIGILL|SIGBUS|ABORT)\s+\[\s*[\d.]+s\]\s+(?:\(\d+/\d+\)\s+)?(\S+)\s+(\S+)\s*$",
    )
});
static BOUNDARY: LazyLock<Regex> = LazyLock::new(|| {
    run_parse::regex(
        r"^\s*(?:TRY \d+ )?(?:PASS|FAIL|SLOW|LEAK|FLAKY|TIMEOUT|SIG[A-Z]+|ABORT)\s+\[|^\s*Summary \[|^\s*Cancelling |^\s*Starting \d+ test|^─+$",
    )
});
static STREAM_HEADER: LazyLock<Regex> =
    LazyLock::new(|| run_parse::regex(r"^\s*(?:stdout|stderr) ─+\s*$"));
static SUMMARY: LazyLock<Regex> = LazyLock::new(|| run_parse::regex(r"^\s*Summary \[.*$"));
static CANCEL: LazyLock<Regex> =
    LazyLock::new(|| run_parse::regex(r"^\s*Cancelling due to (.+?): (\d+) tests? still running"));
const FAILING: &[&str] = &[
    "FAIL", "TIMEOUT", "SIGABRT", "SIGSEGV", "SIGKILL", "SIGILL", "SIGBUS", "ABORT",
];

/// `cargo nextest`: its own status lines across binaries, the panic lifted
/// from the indented stderr block, and no duplicates from the repeat list
/// that follows the summary.
pub struct NextestRunner;

impl RunnerParser for NextestRunner {
    fn name(&self) -> &'static str {
        "nextest"
    }

    fn can_parse(&self, command_line: &str) -> bool {
        run_parse::has_token(command_line, "cargo") && run_parse::has_token(command_line, "nextest")
    }

    fn parse_output(&self, output: &RunOutput) -> Option<RunReport> {
        let lines = run_parse::split_lines(&format!("{}\n{}", output.stdout, output.stderr));
        let mut failures = Vec::new();
        let mut seen = HashSet::new();
        let mut summary: Option<String> = None;
        let mut cancelled: Option<String> = None;
        let mut in_repeat_list = false;
        for (index, line) in lines.iter().enumerate() {
            if SUMMARY.is_match(line) {
                summary = Some(run_parse::collapse_whitespace(line.trim()));
                in_repeat_list = true;
                continue;
            }
            if let Some(cancel) = CANCEL.captures(line) {
                cancelled = Some(format!("{} still running", &cancel[2]));
                continue;
            }
            if in_repeat_list {
                continue;
            }
            let Some(status) = STATUS.captures(line) else {
                continue;
            };
            // TRY lines: wait for the final status.
            if status.get(1).is_some() {
                continue;
            }
            let verdict = &status[2];
            if !FAILING.contains(&verdict) {
                continue;
            }
            let test_name = format!("{} {}", &status[3], &status[4]);
            if !seen.insert(test_name.clone()) {
                continue;
            }
            let block: Vec<String> = lines
                .iter()
                .skip(index + 1)
                .take_while(|body| !BOUNDARY.is_match(body))
                .filter(|body| !STREAM_HEADER.is_match(body))
                .map(|body| body.strip_prefix("    ").unwrap_or(body).to_owned())
                .collect();
            let details = parse_panic_block(&block);
            failures.push(RunFailure {
                test_name,
                file: details.file,
                assertion: if details.assertion.is_empty() {
                    verdict.to_owned()
                } else {
                    details.assertion
                },
                trace: details.trace,
            });
        }
        if let (Some(text), Some(cancelled)) = (summary.as_mut(), cancelled) {
            text.push_str(&format!(" (cancelled: {cancelled})"));
        }
        if failures.is_empty() && output.exit_code != 0 {
            return None;
        }
        Some(RunReport { failures, summary })
    }
}
