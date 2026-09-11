use std::time::Duration;

use serde::Serialize;
use tokio_util::sync::CancellationToken;

use super::{
    Diagnostic, DiagnosticExtractor, PluginRunnerLane, RunExecution, RunExecutor, RunInput,
    RunLimits, RunOutput, RunReport, RunRow, RunnerParser, bundled_extractors, bundled_runners,
    cargo_diagnostics::{order_diagnostics, summarize_diagnostics},
    head_tail::head_tail,
};
use crate::{
    contracts::{
        Budget, EnvelopeVersion, ErrorKind, Freshness, ResultSet, TruncationReason, WorkspaceError,
        WorkspaceResponse,
    },
    query::budget,
    workspace_policy::WorkspacePolicy,
};

const CHARS_PER_TOKEN: u64 = 4;
/// Fallback line allocation: most of the budget to stdout, a slice to stderr.
const STDERR_SHARE: f64 = 0.25;

/// The declared-command use case for one admitted workspace. Admission is
/// the policy's [`crate::config::CommandAdmission`] and nothing here adds to
/// it; execution is the injected [`RunExecutor`]; extraction consults the
/// host-side diagnostic extractors, then plugin runner lanes, then the
/// bundled runner parsers — every registry an open, trust-ordered list —
/// and falls back to an honest head+tail cut with a manifest.
pub struct WorkspaceRun<'a> {
    policy: &'a WorkspacePolicy,
    executor: &'a dyn RunExecutor,
    runners: Vec<Box<dyn RunnerParser>>,
    extractors: Vec<Box<dyn DiagnosticExtractor>>,
    plugin_lanes: Vec<&'a dyn PluginRunnerLane>,
    capture_bytes: usize,
}

impl<'a> WorkspaceRun<'a> {
    #[must_use]
    pub fn new(policy: &'a WorkspacePolicy, executor: &'a dyn RunExecutor) -> Self {
        Self {
            policy,
            executor,
            runners: bundled_runners(),
            extractors: bundled_extractors(),
            plugin_lanes: Vec::new(),
            capture_bytes: RunLimits::CAPTURE_DEFAULT_BYTES,
        }
    }

    /// Replace the runner registry (trust order preserved as given).
    #[must_use]
    pub fn with_runners(mut self, runners: Vec<Box<dyn RunnerParser>>) -> Self {
        self.runners = runners;
        self
    }

    #[must_use]
    pub fn with_extractors(mut self, extractors: Vec<Box<dyn DiagnosticExtractor>>) -> Self {
        self.extractors = extractors;
        self
    }

    /// Plugin runner lanes consulted before the bundled parsers.
    #[must_use]
    pub fn with_plugin_lanes(mut self, lanes: Vec<&'a dyn PluginRunnerLane>) -> Self {
        self.plugin_lanes = lanes;
        self
    }

    #[must_use]
    pub fn with_capture_bytes(mut self, capture_bytes: usize) -> Self {
        self.capture_bytes = capture_bytes;
        self
    }

    pub async fn run(
        &self,
        input: &RunInput,
        cancel: CancellationToken,
    ) -> WorkspaceResponse<RunRow> {
        if let Err(error) = input.validate() {
            return budget::failure(error);
        }
        let argv = match self.policy.admit_command(&input.command, &input.args) {
            Ok(argv) => argv,
            Err(error) => return budget::failure(error),
        };
        let Some(resolved_line) = self.policy.config().commands.get(&input.command).cloned() else {
            return budget::failure(WorkspaceError::new(
                ErrorKind::CommandNotDeclared,
                format!(
                    "command {:?} is not in the merged [commands] table",
                    input.command
                ),
            ));
        };
        let limits = RunLimits::new(Duration::from_secs(input.timeout_secs))
            .with_capture_bytes(self.capture_bytes);
        let execution = match self.executor.execute(argv, limits, cancel).await {
            Ok(execution) => execution,
            Err(error) => return budget::failure(error),
        };
        let mut notes = execution_notes(&execution, input.timeout_secs);
        if execution.is_complete() {
            let output = RunOutput {
                command: resolved_line.clone(),
                exit_code: execution.exit_code.unwrap_or_default(),
                stdout: execution.stdout.clone(),
                stderr: execution.stderr.clone(),
            };
            for extractor in &self.extractors {
                if !extractor.can_extract(&resolved_line) {
                    continue;
                }
                let Some(diagnostics) = extractor.extract(&output) else {
                    break;
                };
                return diagnostics_row(
                    input,
                    &resolved_line,
                    extractor.name(),
                    &execution,
                    diagnostics,
                    notes,
                );
            }
            if let Some(response) = self
                .structured(input, &resolved_line, &execution, &output, &mut notes)
                .await
            {
                return response;
            }
        }
        output_row(input, &resolved_line, &execution, notes)
    }

    /// Plugin lanes first, then the bundled parsers: the first claimant
    /// decides, and a claimant that cannot extract structure ends the search
    /// so the answer is the honest cut rather than a weaker parser's guess.
    async fn structured(
        &self,
        input: &RunInput,
        resolved_line: &str,
        execution: &RunExecution,
        output: &RunOutput,
        notes: &mut Vec<String>,
    ) -> Option<WorkspaceResponse<RunRow>> {
        for lane in &self.plugin_lanes {
            match lane.parse(output.clone()).await {
                Ok(Some(report)) => {
                    return Some(report_row(
                        input,
                        resolved_line,
                        lane.name(),
                        execution,
                        report,
                        notes.clone(),
                    ));
                }
                Ok(None) => return None,
                Err(error) if error.kind == ErrorKind::CommandNotDeclared => {}
                Err(error) => notes.push(format!(
                    "runner plugin {}: {error}; falling back to the bundled parsers",
                    lane.name()
                )),
            }
        }
        for runner in &self.runners {
            if !runner.can_parse(resolved_line) {
                continue;
            }
            let report = runner.parse_output(output)?;
            return Some(report_row(
                input,
                resolved_line,
                runner.name(),
                execution,
                report,
                notes.clone(),
            ));
        }
        None
    }
}

fn execution_notes(execution: &RunExecution, timeout_secs: u64) -> Vec<String> {
    let mut notes = Vec::new();
    if execution.timed_out {
        notes.push(format!(
            "command timed out after {timeout_secs}s and was killed; output below is partial"
        ));
    }
    if execution.cancelled {
        notes.push(
            "command was cancelled and its process group was killed; output below is partial"
                .into(),
        );
    }
    if let Some(signal) = execution.signal {
        notes.push(format!(
            "command was ended by signal {signal}; no exit code"
        ));
    }
    if execution.dropped_bytes > 0 {
        notes.push(format!(
            "{} bytes of output exceeded the capture ceiling and were dropped between the head and the tail of each stream",
            execution.dropped_bytes
        ));
    }
    notes
}

fn report_row(
    input: &RunInput,
    resolved_line: &str,
    adapter: &str,
    execution: &RunExecution,
    report: RunReport,
    notes: Vec<String>,
) -> WorkspaceResponse<RunRow> {
    let row = RunRow::Report {
        command: input.command.clone(),
        resolved_line: resolved_line.to_owned(),
        adapter: adapter.to_owned(),
        exit_code: execution.exit_code,
        timed_out: false,
        duration_ms: execution.duration_ms,
        failures: report.failures,
        summary: report.summary,
    };
    envelope(row, input.budget, None, notes)
}

/// Keep as many ordered diagnostics as the budget allows — at least one when
/// there are any — measuring the row the way every run row is measured.
fn diagnostics_row(
    input: &RunInput,
    resolved_line: &str,
    extractor: &str,
    execution: &RunExecution,
    diagnostics: Vec<Diagnostic>,
    notes: Vec<String>,
) -> WorkspaceResponse<RunRow> {
    let summary = summarize_diagnostics(&diagnostics);
    let ordered = order_diagnostics(diagnostics);
    let base = RunRow::Diagnostics {
        command: input.command.clone(),
        resolved_line: resolved_line.to_owned(),
        extractor: extractor.to_owned(),
        exit_code: execution.exit_code,
        timed_out: false,
        duration_ms: execution.duration_ms,
        diagnostics: Vec::new(),
        summary,
    };
    let limit = input.budget.saturating_mul(CHARS_PER_TOKEN);
    let mut size = wire_chars(&base);
    let mut kept = 0;
    for diagnostic in &ordered {
        let next = size + wire_chars(diagnostic) + 1;
        if next > limit && kept > 0 {
            break;
        }
        size = next;
        kept += 1;
    }
    let dropped = ordered.len() - kept;
    let RunRow::Diagnostics {
        command,
        resolved_line,
        extractor,
        exit_code,
        timed_out,
        duration_ms,
        summary,
        ..
    } = base
    else {
        unreachable!("base is a diagnostics row");
    };
    let row = RunRow::Diagnostics {
        command,
        resolved_line,
        extractor,
        exit_code,
        timed_out,
        duration_ms,
        diagnostics: ordered.into_iter().take(kept).collect(),
        summary,
    };
    let cut = (dropped > 0).then(|| {
        (
            dropped,
            vec![
                "raise budget to keep more diagnostics".to_owned(),
                "fix the errors first — warnings were dropped first, then the latest errors"
                    .to_owned(),
            ],
            format!("diagnostic:{kept}"),
        )
    });
    envelope(row, input.budget, cut, notes)
}

/// Head+tail fallback with a truncation manifest — never a silent guess.
fn output_row(
    input: &RunInput,
    resolved_line: &str,
    execution: &RunExecution,
    notes: Vec<String>,
) -> WorkspaceResponse<RunRow> {
    let total_lines = ((input.budget * CHARS_PER_TOKEN) / 80).max(8) as usize;
    let stderr_lines = ((total_lines as f64 * STDERR_SHARE).floor() as usize).max(4);
    let stdout_lines = total_lines.saturating_sub(stderr_lines).max(4);
    let out = head_tail(
        &execution.stdout,
        stdout_lines.div_ceil(2),
        stdout_lines / 2,
    );
    let err = head_tail(
        &execution.stderr,
        stderr_lines.div_ceil(2),
        stderr_lines / 2,
    );
    let dropped = out.dropped + err.dropped;
    let cursor = format!("line:{}", out.head.len());
    let row = RunRow::Output {
        command: input.command.clone(),
        resolved_line: resolved_line.to_owned(),
        exit_code: execution.exit_code,
        timed_out: execution.timed_out,
        duration_ms: execution.duration_ms,
        stdout_head: out.head,
        stdout_tail: out.tail,
        stderr_head: err.head,
        stderr_tail: err.tail,
    };
    let cut = (dropped > 0).then(|| {
        (
            dropped,
            vec![
                "raise budget to widen the head+tail window".to_owned(),
                "no bundled runner adapter claimed this command — a runner-adapter plugin (§8.3) can add structured extraction".to_owned(),
            ],
            cursor,
        )
    });
    envelope(row, input.budget, cut, notes)
}

fn envelope(
    row: RunRow,
    requested: u64,
    cut: Option<(usize, Vec<String>, String)>,
    notes: Vec<String>,
) -> WorkspaceResponse<RunRow> {
    let used = match budget::tokens(&row) {
        Ok(used) => used,
        Err(error) => return budget::failure(error),
    };
    let results = match cut {
        None => ResultSet::complete(vec![row]),
        Some((dropped, narrow, cursor)) => {
            match ResultSet::with_manifest(
                vec![row],
                dropped,
                TruncationReason::Budget,
                narrow,
                cursor,
            ) {
                Ok(results) => results,
                Err(error) => return budget::failure(error),
            }
        }
    };
    WorkspaceResponse::Success {
        version: EnvelopeVersion::LATEST,
        freshness: Freshness::Warm,
        budget: Budget { requested, used },
        results,
        notes: (!notes.is_empty()).then_some(notes),
    }
}

/// The retained wire measure: JavaScript string length, hence UTF-16 units.
fn wire_chars<T: Serialize>(value: &T) -> u64 {
    serde_json::to_string(value)
        .map(|text| text.encode_utf16().count() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
#[path = "run_service.tests.rs"]
mod tests;
