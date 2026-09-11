use std::{collections::HashSet, sync::LazyLock};

use regex::Regex;
use serde_json::Value;

use super::{
    Diagnostic, DiagnosticExtractor, DiagnosticLevel, RunOutput,
    cargo_subcommand::{cargo_subcommand, uses_json_messages},
    run_parse,
};

const DETAIL_MAX_LINES: usize = 12;
const DIAGNOSTIC_SUBCOMMANDS: &[&str] = &["build", "check", "clippy", "test", "nextest"];

static ARROW: LazyLock<Regex> = LazyLock::new(|| run_parse::regex(r"^\s*--> "));
static LINT_ATTRIBUTE: LazyLock<Regex> =
    LazyLock::new(|| run_parse::regex(r"`?#\[(?:warn|deny|forbid|expect)\(([A-Za-z0-9_:]+)\)\]"));
static RENDERED_HEADER: LazyLock<Regex> =
    LazyLock::new(|| run_parse::regex(r"^(error|warning)(?:\[([A-Za-z0-9_:]+)\])?: (.+)$"));
static LOCATION: LazyLock<Regex> =
    LazyLock::new(|| run_parse::regex(r"^\s*--> (.+?):(\d+):(\d+)\s*$"));
/// Lines cargo prints ABOUT a build rather than diagnostics of it.
static CARGO_SUMMARY: LazyLock<Regex> = LazyLock::new(|| {
    run_parse::regex(
        r"^could not compile(?-u:\b)|^aborting due to(?-u:\b)|^build failed(?-u:\b)|generated \d+ warnings?(?-u:\b)|^test failed(?-u:\b)|^\d+ targets? failed(?-u:\b)|^test run failed(?-u:\b)",
    )
});
static TEST_EXECUTION: LazyLock<Regex> = LazyLock::new(|| {
    run_parse::regex(
        r"(?m)^\s*running \d+ tests?$|^\s*test .+? \.\.\. |^\s*Starting \d+ tests? across|^\s*(?:PASS|FAIL) \[",
    )
});

/// The host-side extractor that lifts rustc/clippy diagnostics — from
/// `--message-format=json` or the rendered form, equal rows either way —
/// into `run`'s diagnostics row. Test commands are claimed only when the
/// build failed: error-level diagnostics and no test execution at all.
pub struct CargoDiagnostics;

impl DiagnosticExtractor for CargoDiagnostics {
    fn name(&self) -> &'static str {
        "cargo"
    }

    fn can_extract(&self, command_line: &str) -> bool {
        run_parse::has_token(command_line, "cargo")
            && cargo_subcommand(command_line)
                .is_some_and(|sub| DIAGNOSTIC_SUBCOMMANDS.contains(&sub.as_str()))
    }

    fn extract(&self, output: &RunOutput) -> Option<Vec<Diagnostic>> {
        extract_cargo_diagnostics(output)
    }
}

pub(super) fn extract_cargo_diagnostics(output: &RunOutput) -> Option<Vec<Diagnostic>> {
    let sub = cargo_subcommand(&output.command)?;
    if !DIAGNOSTIC_SUBCOMMANDS.contains(&sub.as_str()) {
        return None;
    }
    let raw = if uses_json_messages(&output.command) {
        let mut raw = parse_json_diagnostics(&output.stdout);
        raw.extend(parse_rendered_diagnostics(&output.stderr));
        raw
    } else {
        parse_rendered_diagnostics(&format!("{}\n{}", output.stdout, output.stderr))
    };
    let diagnostics = order_diagnostics(dedupe(raw));
    if sub == "test" || sub == "nextest" {
        let compile_failed = diagnostics
            .iter()
            .any(|diagnostic| diagnostic.level == DiagnosticLevel::Error)
            && !TEST_EXECUTION.is_match(&format!("{}\n{}", output.stdout, output.stderr));
        return compile_failed.then_some(diagnostics);
    }
    if diagnostics.is_empty() && output.exit_code != 0 {
        return None;
    }
    Some(diagnostics)
}

/// Errors first (emission order), then warnings.
pub(super) fn order_diagnostics(diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    let (errors, warnings): (Vec<_>, Vec<_>) = diagnostics
        .into_iter()
        .partition(|diagnostic| diagnostic.level == DiagnosticLevel::Error);
    errors.into_iter().chain(warnings).collect()
}

/// "2 errors, 1 warning" | "no diagnostics".
pub(super) fn summarize_diagnostics(diagnostics: &[Diagnostic]) -> String {
    let errors = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.level == DiagnosticLevel::Error)
        .count();
    let warnings = diagnostics.len() - errors;
    if diagnostics.is_empty() {
        return "no diagnostics".into();
    }
    let mut parts = Vec::new();
    if errors > 0 {
        parts.push(format!(
            "{errors} error{}",
            if errors == 1 { "" } else { "s" }
        ));
    }
    if warnings > 0 {
        parts.push(format!(
            "{warnings} warning{}",
            if warnings == 1 { "" } else { "s" }
        ));
    }
    parts.join(", ")
}

/// The rendered text minus its header and `-->` lines: the snippet plus
/// help/note lines, capped. Shared by both modes so they stay equal.
fn detail_of(lines: &[String]) -> String {
    let body: Vec<&str> = lines.iter().map(|line| line.trim_end()).collect();
    let start = usize::from(body.first().is_some_and(|first| ARROW.is_match(first)));
    let mut kept = body[start.min(body.len())..].to_vec();
    while kept.last().is_some_and(|last| last.trim().is_empty()) {
        kept.pop();
    }
    kept.truncate(DETAIL_MAX_LINES);
    kept.join("\n")
}

fn level_of(level: &str) -> Option<DiagnosticLevel> {
    match level {
        "error" => Some(DiagnosticLevel::Error),
        "warning" => Some(DiagnosticLevel::Warning),
        _ => None,
    }
}

fn parse_json_diagnostics(stdout: &str) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for raw in run_parse::split_lines(stdout) {
        let line = raw.trim();
        if !line.starts_with('{') {
            continue;
        }
        let Ok(parsed) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if parsed.get("reason").and_then(Value::as_str) != Some("compiler-message") {
            continue;
        }
        let Some(message) = parsed.get("message") else {
            continue;
        };
        let Some(level) = message
            .get("level")
            .and_then(Value::as_str)
            .and_then(level_of)
        else {
            continue;
        };
        let spans = message.get("spans").and_then(Value::as_array);
        let span = spans.and_then(|spans| {
            spans
                .iter()
                .find(|span| span.get("is_primary").and_then(Value::as_bool) == Some(true))
                .or_else(|| spans.first())
        });
        let detail = match message.get("rendered").and_then(Value::as_str) {
            Some(rendered) => detail_of(&run_parse::split_lines(rendered)[1..]),
            None => message
                .get("children")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|child| {
                    child.get("message").and_then(Value::as_str).map(|text| {
                        format!(
                            "{}: {text}",
                            child.get("level").and_then(Value::as_str).unwrap_or("note")
                        )
                    })
                })
                .collect::<Vec<_>>()
                .join("\n"),
        };
        out.push(Diagnostic {
            level,
            code: message
                .get("code")
                .and_then(|code| code.get("code"))
                .and_then(Value::as_str)
                .map(str::to_owned),
            message: message
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            file: span
                .and_then(|span| span.get("file_name"))
                .and_then(Value::as_str)
                .map(run_parse::normalize_path),
            line: span
                .and_then(|span| span.get("line_start"))
                .and_then(Value::as_u64),
            column: span
                .and_then(|span| span.get("column_start"))
                .and_then(Value::as_u64),
            detail,
        });
    }
    out
}

fn parse_rendered_diagnostics(text: &str) -> Vec<Diagnostic> {
    let lines = run_parse::split_lines(text);
    let mut out = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let Some(header) = RENDERED_HEADER.captures(&lines[index]) else {
            index += 1;
            continue;
        };
        let message = header[3].trim().to_owned();
        if CARGO_SUMMARY.is_match(&message) {
            index += 1;
            continue;
        }
        let block: Vec<String> = lines
            .iter()
            .skip(index + 1)
            .take_while(|body| !(body.trim().is_empty() || RENDERED_HEADER.is_match(body)))
            .cloned()
            .collect();
        let location = block.first().and_then(|first| LOCATION.captures(first));
        let code = header
            .get(2)
            .map(|code| code.as_str().to_owned())
            .or_else(|| {
                block
                    .iter()
                    .find_map(|body| LINT_ATTRIBUTE.captures(body).map(|attr| attr[1].to_owned()))
            });
        out.push(Diagnostic {
            level: if &header[1] == "error" {
                DiagnosticLevel::Error
            } else {
                DiagnosticLevel::Warning
            },
            code,
            message,
            file: location
                .as_ref()
                .map(|location| run_parse::normalize_path(&location[1])),
            line: location
                .as_ref()
                .and_then(|location| location[2].parse().ok()),
            column: location
                .as_ref()
                .and_then(|location| location[3].parse().ok()),
            detail: detail_of(&block),
        });
        index += block.len() + 1;
    }
    out
}

fn dedupe(diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    let mut seen = HashSet::new();
    diagnostics
        .into_iter()
        .filter(|diagnostic| {
            seen.insert(format!(
                "{:?}\0{}\0{}\0{}\0{}\0{}",
                diagnostic.level,
                diagnostic.code.as_deref().unwrap_or_default(),
                diagnostic.message,
                diagnostic.file.as_deref().unwrap_or_default(),
                diagnostic
                    .line
                    .map(|line| line.to_string())
                    .unwrap_or_default(),
                diagnostic
                    .column
                    .map(|column| column.to_string())
                    .unwrap_or_default()
            ))
        })
        .collect()
}
