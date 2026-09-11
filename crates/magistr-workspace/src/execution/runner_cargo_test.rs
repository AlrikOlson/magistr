use std::{collections::HashMap, sync::LazyLock};

use regex::Regex;

use super::{
    RunFailure, RunOutput, RunReport, RunnerParser,
    cargo_panic::{PanicDetails, parse_panic_block},
    cargo_subcommand::cargo_subcommand,
    run_parse,
};

static RUNNING_BINARY: LazyLock<Regex> = LazyLock::new(|| {
    run_parse::regex(r"^\s*Running .*[\\/]deps[\\/]([A-Za-z0-9_.-]+?)-[0-9a-f]+(?:\.exe)?\)\s*$")
});
static DOC_TESTS: LazyLock<Regex> = LazyLock::new(|| run_parse::regex(r"^\s*Doc-tests (\S+)\s*$"));
/// libtest's summary line; `ignored` onward is optional so a truncated line
/// still counts.
static TEST_RESULT: LazyLock<Regex> = LazyLock::new(|| {
    run_parse::regex(r"^test result: (\w+)\. (\d+) passed; (\d+) failed(?:; (\d+) ignored)?")
});
static RUNNING_HEADER: LazyLock<Regex> =
    LazyLock::new(|| run_parse::regex(r"^running \d+ tests?$"));
static FAILED_LINE: LazyLock<Regex> =
    LazyLock::new(|| run_parse::regex(r"^test\s+(.+?)\s+\.\.\.\s+FAILED$"));
static BLOCK_HEADER: LazyLock<Regex> =
    LazyLock::new(|| run_parse::regex(r"^----\s+(.+?)\s+stdout\s+----$"));
static FAILURES_LINE: LazyLock<Regex> = LazyLock::new(|| run_parse::regex(r"^failures:$"));
static RERUN_HINT: LazyLock<Regex> = LazyLock::new(|| run_parse::regex(r"to rerun pass `([^`]+)`"));

/// `cargo test` (libtest output). cargo prints its `Running <binary>` /
/// `Doc-tests <crate>` status lines on STDERR and libtest's output on
/// STDOUT, so when the streams are captured separately the two never
/// interleave. Attribution therefore pairs them by ORDER: every
/// `running N tests` header on stdout opens the next binary's section, and
/// the i-th section belongs to the i-th status line. The same pairing holds
/// for a `2>&1` capture, where they do interleave.
pub struct CargoTestRunner;

impl RunnerParser for CargoTestRunner {
    fn name(&self) -> &'static str {
        "cargo"
    }

    fn can_parse(&self, command_line: &str) -> bool {
        run_parse::has_token(command_line, "cargo")
            && cargo_subcommand(command_line).as_deref() == Some("test")
    }

    fn parse_output(&self, output: &RunOutput) -> Option<RunReport> {
        let lines = run_parse::split_lines(&format!("{}\n{}", output.stdout, output.stderr));
        let mut binaries: Vec<String> = Vec::new();
        let mut failed: Vec<(usize, String)> = Vec::new();
        let mut details: HashMap<(usize, String), PanicDetails> = HashMap::new();
        let mut results: Vec<String> = Vec::new();
        let mut hints: Vec<String> = Vec::new();
        let (mut passed, mut failed_total, mut ignored) = (0u64, 0u64, 0u64);
        let mut section: Option<usize> = None;
        let current = |section: Option<usize>| section.unwrap_or(0);
        for (index, line) in lines.iter().enumerate() {
            if let Some(running) = RUNNING_BINARY.captures(line) {
                binaries.push(running[1].to_owned());
                continue;
            }
            if let Some(doc) = DOC_TESTS.captures(line) {
                binaries.push(format!("{} (doctest)", &doc[1]));
                continue;
            }
            if RUNNING_HEADER.is_match(line.trim()) {
                section = Some(section.map_or(0, |current| current + 1));
                continue;
            }
            if let Some(failed_line) = FAILED_LINE.captures(line) {
                failed.push((current(section), failed_line[1].trim().to_owned()));
                continue;
            }
            if let Some(header) = BLOCK_HEADER.captures(line) {
                let block: Vec<String> = lines
                    .iter()
                    .skip(index + 1)
                    .take_while(|body| {
                        !BLOCK_HEADER.is_match(body) && !FAILURES_LINE.is_match(body)
                    })
                    .cloned()
                    .collect();
                details.insert(
                    (current(section), header[1].trim().to_owned()),
                    parse_panic_block(&block),
                );
                continue;
            }
            if let Some(result) = TEST_RESULT.captures(line) {
                results.push(line.trim().to_owned());
                passed += number(result.get(2));
                failed_total += number(result.get(3));
                ignored += number(result.get(4));
                continue;
            }
            if let Some(hint) = RERUN_HINT.captures(line) {
                hints.push(hint[1].to_owned());
            }
        }
        let several = section.is_some_and(|section| section > 0);
        let failures: Vec<RunFailure> = failed
            .into_iter()
            .map(|(binary, name)| {
                let detail = details.get(&(binary, name.clone()));
                // A section without a status line (a binary that died before
                // cargo's line was captured) is named by its position.
                let stem = binaries
                    .get(binary)
                    .cloned()
                    .unwrap_or_else(|| format!("binary {}", binary + 1));
                RunFailure {
                    test_name: if several {
                        format!("{stem} {name}")
                    } else {
                        name
                    },
                    file: detail.and_then(|detail| detail.file.clone()),
                    assertion: detail
                        .map(|detail| detail.assertion.clone())
                        .unwrap_or_default(),
                    trace: detail
                        .map(|detail| detail.trace.clone())
                        .unwrap_or_default(),
                }
            })
            .collect();
        let summary = match results.len() {
            0 => None,
            1 => results.pop(),
            count => {
                let mut summary = format!(
                    "{passed} passed; {failed_total} failed; {ignored} ignored across {count} binaries"
                );
                if !hints.is_empty() {
                    summary.push_str(&format!(" — rerun: {}", hints.join(", ")));
                }
                Some(summary)
            }
        };
        if failures.is_empty() && output.exit_code != 0 {
            return None;
        }
        Some(RunReport { failures, summary })
    }
}

fn number(capture: Option<regex::Match<'_>>) -> u64 {
    capture
        .and_then(|value| value.as_str().parse().ok())
        .unwrap_or(0)
}
