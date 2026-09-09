//! Structured failure rows from quality-gate output.
//!
//! workbench-mcp's `run` tool turns a runner's output into rows its clients
//! can click on (`src/tools/run-cargo.ts`, `run-parse.ts`); Magistr cannot
//! delegate its gates to that server (it is single-instance per workspace),
//! so the same adapters live here, in Rust, over the output `magistr_gate`
//! already captures. Pure functions from text to rows; nothing here spawns.
//!
//! Formats:
//! - rustc / clippy diagnostics, JSON (`--message-format=json` compiler
//!   messages) or rendered (`error[E0308]: …` + `--> file:line:col`);
//! - `cargo nextest` failures (status line + panic block);
//! - `cargo test` (libtest) failures (`test … FAILED` + `---- … stdout ----`);
//! - a junit report (`<testcase>` + `<failure>`), the file `ship_check`'s
//!   `report` argument points at;
//! - a generic `path:line[:col]: message` / `path(line,col): message` line
//!   for every other tool (eslint, tsc, pytest, mypy, …).

use serde::{Deserialize, Serialize};

/// What kind of failure a row is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateRowSeverity {
    /// A compiler / linter error or an unrecognized failing line.
    Error,
    /// A failing test.
    TestFailure,
    /// A compiler / linter warning.
    Warning,
}

/// One failure a gate reported, with the location a client can open.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateFailureRow {
    pub severity: GateRowSeverity,
    /// Diagnostic code (`E0308`, `clippy::needless_return`, `TS2322`), when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// The headline: the diagnostic message, or `test name: assertion`.
    pub message: String,
    /// Workspace-relative path with forward slashes, when the output named one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
}

impl GateFailureRow {
    /// `file:line:col` (as much of it as is known), or `None` without a file.
    #[must_use]
    pub fn location(&self) -> Option<String> {
        let file = self.file.as_ref()?;
        let mut loc = file.clone();
        if let Some(line) = self.line {
            loc.push(':');
            loc.push_str(&line.to_string());
            if let Some(col) = self.column {
                loc.push(':');
                loc.push_str(&col.to_string());
            }
        }
        Some(loc)
    }
}

/// Parse a gate's captured output (stdout + stderr, in any order) into rows,
/// choosing the adapters by the command that produced it.
#[must_use]
pub fn parse_gate_rows(command: &str, output: &str) -> Vec<GateFailureRow> {
    let text = output.replace("\r\n", "\n");
    let mut rows = Vec::new();
    match cargo_subcommand(command) {
        Some(sub) => {
            rows.extend(parse_json_diagnostics(&text));
            rows.extend(parse_rendered_diagnostics(&text));
            match sub.as_str() {
                "nextest" => rows.extend(parse_nextest_failures(&text)),
                "test" => rows.extend(parse_libtest_failures(&text)),
                _ => {}
            }
        }
        None => {
            rows.extend(parse_rendered_diagnostics(&text));
            rows.extend(parse_generic_rows(&text));
        }
    }
    order(dedupe(rows))
}

/// Failing test cases of a junit report, one row each.
#[must_use]
pub fn junit_failure_rows(xml: &str) -> Vec<GateFailureRow> {
    let mut rows = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<testcase") {
        let case = &rest[start..];
        let Some(tag_end) = case.find('>') else { break };
        let open_tag = &case[..tag_end + 1];
        let self_closing = open_tag.ends_with("/>");
        let body_end = if self_closing {
            tag_end + 1
        } else {
            case.find("</testcase>")
                .map(|i| i + "</testcase>".len())
                .unwrap_or(case.len())
        };
        let element = &case[..body_end];
        let failure = element.find("<failure").or_else(|| element.find("<error"));
        if let Some(at) = failure {
            let failure_tag = &element[at..];
            let failure_tag = &failure_tag[..failure_tag.find('>').unwrap_or(failure_tag.len())];
            let name = xml_attr(open_tag, "name").unwrap_or_default();
            let classname = xml_attr(open_tag, "classname");
            let full = match classname {
                Some(c) if !c.is_empty() && !name.starts_with(&c) => format!("{c}::{name}"),
                _ => name,
            };
            let message = match xml_attr(failure_tag, "message") {
                Some(m) if !m.is_empty() => format!("{full}: {}", first_line(&m)),
                _ => full,
            };
            rows.push(GateFailureRow {
                severity: GateRowSeverity::TestFailure,
                code: None,
                message,
                file: None,
                line: None,
                column: None,
            });
        }
        rest = &rest[start + body_end..];
    }
    rows
}

// ── command facts ─────────────────────────────────────────────────────────

/// Program-name normalization: strip the path (both separators) and `.exe`.
fn program_basename(token: &str) -> String {
    let base = token.rsplit(['/', '\\']).next().unwrap_or(token);
    base.to_ascii_lowercase()
        .trim_end_matches(".exe")
        .to_owned()
}

/// The cargo subcommand of a command line (`cargo +nightly clippy …` → `clippy`).
fn cargo_subcommand(command: &str) -> Option<String> {
    let tokens: Vec<String> = command.split_whitespace().map(program_basename).collect();
    let at = tokens.iter().position(|t| t == "cargo")?;
    tokens[at + 1..]
        .iter()
        .find(|t| !t.starts_with('+') && !t.starts_with('-'))
        .cloned()
}

// ── shared helpers ────────────────────────────────────────────────────────

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or_default().trim().to_owned()
}

/// Lines cargo prints ABOUT a build rather than diagnostics of it.
fn is_cargo_summary(message: &str) -> bool {
    message.starts_with("could not compile")
        || message.starts_with("aborting due to")
        || message.starts_with("build failed")
        || message.contains("generated ") && message.contains("warning")
        || message.starts_with("test failed")
        || message.starts_with("test run failed")
        || message.starts_with("process didn't exit successfully")
        || message.starts_with("failed to run custom build command")
        || (message.contains("target") && message.ends_with("failed"))
}

/// `--> path:line:col` → (path, line, col).
fn parse_arrow_location(line: &str) -> Option<(String, u32, u32)> {
    let rest = line.trim().strip_prefix("--> ")?;
    parse_path_line_col(rest.trim())
}

/// `path:line:col` (col optional) → (path, line, col-or-0).
fn parse_path_line_col(s: &str) -> Option<(String, u32, u32)> {
    let s = s.trim_end_matches(':');
    let mut parts = s.rsplitn(3, ':');
    let last = parts.next()?;
    let middle = parts.next()?;
    match (last.parse::<u32>(), middle.parse::<u32>()) {
        (Ok(col), Ok(line)) => {
            let path = parts.next()?;
            (!path.is_empty()).then(|| (normalize_path(path), line, col))
        }
        (Ok(line), Err(_)) => {
            // `path:line` — middle is part of the path (or the whole path).
            let path = match parts.next() {
                Some(head) => format!("{head}:{middle}"),
                None => middle.to_owned(),
            };
            (!path.is_empty()).then(|| (normalize_path(&path), line, 0))
        }
        _ => None,
    }
}

fn dedupe(rows: Vec<GateFailureRow>) -> Vec<GateFailureRow> {
    let mut seen = std::collections::HashSet::new();
    rows.into_iter()
        .filter(|r| {
            seen.insert((
                r.severity,
                r.code.clone(),
                r.message.clone(),
                r.file.clone(),
                r.line,
                r.column,
            ))
        })
        .collect()
}

/// Errors first, then test failures, then warnings — emission order within.
fn order(mut rows: Vec<GateFailureRow>) -> Vec<GateFailureRow> {
    rows.sort_by_key(|r| r.severity);
    rows
}

// ── rustc / clippy: JSON ──────────────────────────────────────────────────

fn parse_json_diagnostics(text: &str) -> Vec<GateFailureRow> {
    text.lines()
        .map(str::trim)
        .filter(|l| l.starts_with('{'))
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter(|v| v["reason"] == "compiler-message")
        .filter_map(|v| {
            let msg = &v["message"];
            let severity = match msg["level"].as_str()? {
                "error" => GateRowSeverity::Error,
                "warning" => GateRowSeverity::Warning,
                _ => return None,
            };
            let message = msg["message"].as_str()?.to_owned();
            if is_cargo_summary(&message) {
                return None;
            }
            let spans = msg["spans"].as_array().cloned().unwrap_or_default();
            let span = spans
                .iter()
                .find(|s| s["is_primary"] == true)
                .or_else(|| spans.first());
            Some(GateFailureRow {
                severity,
                code: msg["code"]["code"].as_str().map(str::to_owned),
                message,
                file: span
                    .and_then(|s| s["file_name"].as_str())
                    .map(normalize_path),
                line: span
                    .and_then(|s| s["line_start"].as_u64())
                    .map(|n| n as u32),
                column: span
                    .and_then(|s| s["column_start"].as_u64())
                    .map(|n| n as u32),
            })
        })
        .collect()
}

// ── rustc / clippy: rendered ──────────────────────────────────────────────

/// `error[E0308]: msg` / `warning: msg` → (severity, code, message).
fn parse_rendered_header(line: &str) -> Option<(GateRowSeverity, Option<String>, String)> {
    let (severity, rest) = if let Some(r) = line.strip_prefix("error") {
        (GateRowSeverity::Error, r)
    } else if let Some(r) = line.strip_prefix("warning") {
        (GateRowSeverity::Warning, r)
    } else {
        return None;
    };
    let (code, rest) = match rest.strip_prefix('[') {
        Some(r) => {
            let end = r.find(']')?;
            (Some(r[..end].to_owned()), &r[end + 1..])
        }
        None => (None, rest),
    };
    let message = rest.strip_prefix(": ")?.trim();
    (!message.is_empty()).then(|| (severity, code, message.to_owned()))
}

/// A lint code named in a `#[warn(...)]`-style attribute line.
fn lint_attribute_code(line: &str) -> Option<String> {
    for kw in ["#[warn(", "#[deny(", "#[forbid(", "#[expect("] {
        if let Some(at) = line.find(kw) {
            let rest = &line[at + kw.len()..];
            let end = rest.find(')')?;
            return Some(rest[..end].to_owned());
        }
    }
    None
}

fn parse_rendered_diagnostics(text: &str) -> Vec<GateFailureRow> {
    let lines: Vec<&str> = text.lines().collect();
    let mut rows = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let Some((severity, header_code, message)) = parse_rendered_header(lines[i]) else {
            i += 1;
            continue;
        };
        if is_cargo_summary(&message) {
            i += 1;
            continue;
        }
        let mut block = Vec::new();
        let mut j = i + 1;
        while j < lines.len() {
            let body = lines[j];
            if body.trim().is_empty() || parse_rendered_header(body).is_some() {
                break;
            }
            block.push(body);
            j += 1;
        }
        let location = block.first().and_then(|l| parse_arrow_location(l));
        let code = header_code.or_else(|| block.iter().find_map(|l| lint_attribute_code(l)));
        rows.push(GateFailureRow {
            severity,
            code,
            message,
            file: location.as_ref().map(|(f, _, _)| f.clone()),
            line: location.as_ref().map(|(_, l, _)| *l),
            column: location.as_ref().map(|(_, _, c)| *c).filter(|c| *c > 0),
        });
        i = j.max(i + 1);
    }
    rows
}

// ── the panic block: shared by cargo test and nextest ─────────────────────

struct PanicDetails {
    file: Option<(String, u32, u32)>,
    assertion: String,
}

fn is_panic_noise(line: &str) -> bool {
    line.starts_with("note: run with `RUST_BACKTRACE")
        || line.starts_with("note: Some details are omitted")
        || line == "stack backtrace:"
}

/// Find the `panicked at` line of a test's output block and read its
/// location and message (rustc ≥ 1.73 `panicked at path:line:col:` with the
/// message on the same or next line; older `panicked at 'msg', path:line:col`).
fn parse_panic_block(block: &[&str]) -> PanicDetails {
    for (i, raw) in block.iter().enumerate() {
        let line = raw.trim();
        let Some(at) = line.find("panicked at ") else {
            continue;
        };
        let rest = &line[at + "panicked at ".len()..];
        // Legacy shape first: its quoted message would otherwise read as a
        // path. The message may span lines (`left:`/`right:` continuation);
        // the `', path:line:col` terminator ends it on a later line.
        if let Some(quoted) = rest.strip_prefix('\'') {
            let assertion = match quoted.rfind("', ") {
                Some(end) => quoted[..end].to_owned(),
                None => quoted.to_owned(),
            };
            let location = std::iter::once(quoted)
                .chain(block[i + 1..].iter().map(|l| l.trim()))
                .find_map(|l| l.rfind("', ").map(|end| l[end + 3..].trim().to_owned()));
            return PanicDetails {
                file: location.as_deref().and_then(parse_path_line_col),
                assertion: assertion.trim().to_owned(),
            };
        }
        // Modern shape: `path:line:col:` then the message (same line or next).
        let (loc_part, mut message) = match rest.find(": ") {
            Some(cut) => (&rest[..cut], rest[cut + 2..].trim().to_owned()),
            None => (rest.trim_end_matches(':'), String::new()),
        };
        let file = parse_path_line_col(loc_part);
        if message.is_empty() {
            message = block[i + 1..]
                .iter()
                .map(|l| l.trim())
                .find(|l| !l.is_empty())
                .filter(|l| !is_panic_noise(l))
                .unwrap_or_default()
                .to_owned();
        }
        return PanicDetails {
            file,
            assertion: message,
        };
    }
    PanicDetails {
        file: None,
        assertion: String::new(),
    }
}

fn test_row(name: &str, details: &PanicDetails, fallback: &str) -> GateFailureRow {
    let assertion = if details.assertion.is_empty() {
        fallback
    } else {
        details.assertion.as_str()
    };
    GateFailureRow {
        severity: GateRowSeverity::TestFailure,
        code: None,
        message: format!("{name}: {assertion}"),
        file: details.file.as_ref().map(|(f, _, _)| f.clone()),
        line: details.file.as_ref().map(|(_, l, _)| *l),
        column: details.file.as_ref().map(|(_, _, c)| *c).filter(|c| *c > 0),
    }
}

// ── cargo nextest ─────────────────────────────────────────────────────────

const NEXTEST_FAILING: [&str; 8] = [
    "FAIL", "TIMEOUT", "SIGABRT", "SIGSEGV", "SIGKILL", "SIGILL", "SIGBUS", "ABORT",
];
const NEXTEST_ANY: [&str; 12] = [
    "PASS", "FAIL", "SLOW", "LEAK", "FLAKY", "TIMEOUT", "SIGABRT", "SIGSEGV", "SIGKILL", "SIGILL",
    "SIGBUS", "ABORT",
];

/// `FAIL [ 0.008s] (1/6) binary tests::name` → (verdict, "binary tests::name");
/// `TRY n …` lines (retries) are skipped — the final status follows.
fn parse_nextest_status(line: &str) -> Option<(&str, String)> {
    let mut tokens = line.split_whitespace();
    let mut verdict = tokens.next()?;
    if verdict == "TRY" {
        return None;
    }
    if !NEXTEST_ANY.contains(&verdict) {
        return None;
    }
    // `[   0.008s]` may be split into two tokens by the padding.
    let mut tok = tokens.next()?;
    if !tok.starts_with('[') {
        return None;
    }
    while !tok.ends_with(']') {
        tok = tokens.next()?;
    }
    let mut next = tokens.next()?;
    if next.starts_with('(') {
        next = tokens.next()?;
    }
    let binary = next;
    let name = tokens.next()?;
    if tokens.next().is_some() {
        return None;
    }
    verdict = NEXTEST_ANY.iter().find(|v| **v == verdict)?;
    Some((verdict, format!("{binary} {name}")))
}

fn is_nextest_boundary(line: &str) -> bool {
    let t = line.trim_start();
    parse_nextest_status(t).is_some()
        || t.starts_with("TRY ")
        || t.starts_with("Summary [")
        || t.starts_with("Cancelling ")
        || t.starts_with("Starting ")
        || (!t.is_empty() && t.chars().all(|c| c == '─'))
}

fn parse_nextest_failures(text: &str) -> Vec<GateFailureRow> {
    let lines: Vec<&str> = text.lines().collect();
    let mut rows = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (i, raw) in lines.iter().enumerate() {
        let trimmed = raw.trim_start();
        if trimmed.starts_with("Summary [") {
            break; // the repeat list of failures follows; already collected
        }
        let Some((verdict, name)) = parse_nextest_status(trimmed) else {
            continue;
        };
        if !NEXTEST_FAILING.contains(&verdict) || !seen.insert(name.clone()) {
            continue;
        }
        let block: Vec<&str> = lines[i + 1..]
            .iter()
            .take_while(|l| !is_nextest_boundary(l))
            .filter(|l| {
                let t = l.trim();
                !((t.starts_with("stdout ") || t.starts_with("stderr ")) && t.contains('─'))
            })
            .map(|l| l.strip_prefix("    ").unwrap_or(l))
            .collect();
        let details = parse_panic_block(&block);
        rows.push(test_row(&name, &details, verdict));
    }
    rows
}

// ── cargo test (libtest) ──────────────────────────────────────────────────

fn parse_libtest_failures(text: &str) -> Vec<GateFailureRow> {
    let lines: Vec<&str> = text.lines().collect();
    let mut failed: Vec<String> = Vec::new();
    let mut details: Vec<(String, PanicDetails)> = Vec::new();
    for (i, raw) in lines.iter().enumerate() {
        let line = raw.trim();
        if let Some(rest) = line.strip_prefix("test ")
            && let Some(name) = rest.strip_suffix(" ... FAILED")
        {
            failed.push(name.trim().to_owned());
            continue;
        }
        if let Some(rest) = line.strip_prefix("---- ")
            && let Some(name) = rest.strip_suffix(" stdout ----")
        {
            let block: Vec<&str> = lines[i + 1..]
                .iter()
                .take_while(|l| {
                    let t = l.trim();
                    !(t.starts_with("---- ") && t.ends_with(" stdout ----")) && t != "failures:"
                })
                .copied()
                .collect();
            details.push((name.trim().to_owned(), parse_panic_block(&block)));
        }
    }
    let empty = PanicDetails {
        file: None,
        assertion: String::new(),
    };
    failed
        .iter()
        .map(|name| {
            let d = details
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, d)| d)
                .unwrap_or(&empty);
            test_row(name, d, "FAILED")
        })
        .collect()
}

// ── generic `path:line[:col]: message` ────────────────────────────────────

fn looks_like_path(s: &str) -> bool {
    !s.is_empty()
        && !s.contains(char::is_whitespace)
        && (s.contains('/') || s.contains('\\') || s.contains('.'))
        && !s.starts_with("http")
}

/// `src/a.ts(12,5): message` (tsc) → (path, line, col, message).
fn parse_paren_location(line: &str) -> Option<(String, u32, u32, &str)> {
    let open = line.find('(')?;
    let close = line[open..].find("):")? + open;
    let path = &line[..open];
    if !looks_like_path(path) {
        return None;
    }
    let mut nums = line[open + 1..close].split(',');
    let l: u32 = nums.next()?.trim().parse().ok()?;
    let c: u32 = nums.next().and_then(|c| c.trim().parse().ok()).unwrap_or(0);
    Some((normalize_path(path), l, c, line[close + 2..].trim()))
}

fn parse_generic_rows(text: &str) -> Vec<GateFailureRow> {
    text.lines()
        .filter_map(|raw| {
            let line = raw.trim();
            if line.is_empty() || parse_rendered_header(line).is_some() {
                return None;
            }
            let (file, l, c, message) = if let Some(p) = parse_paren_location(line) {
                p
            } else {
                // The location ends at the first `: ` whose left side parses.
                let mut found = None;
                let mut search_from = 0;
                while let Some(rel) = line[search_from..].find(": ") {
                    let cut = search_from + rel;
                    if let Some((f, l, c)) = parse_path_line_col(&line[..cut])
                        && looks_like_path(&f)
                    {
                        found = Some((f, l, c, line[cut + 2..].trim()));
                        break;
                    }
                    search_from = cut + 2;
                }
                found?
            };
            if message.is_empty() {
                return None;
            }
            let lower = message.to_ascii_lowercase();
            let severity = if lower.starts_with("warning") {
                GateRowSeverity::Warning
            } else {
                GateRowSeverity::Error
            };
            let (code, message) = split_generic_code(message);
            Some(GateFailureRow {
                severity,
                code,
                message,
                file: Some(file),
                line: Some(l),
                column: (c > 0).then_some(c),
            })
        })
        .collect()
}

/// `error TS2322: msg` → (Some("TS2322"), "msg"); `error: msg` → (None, "msg").
fn split_generic_code(message: &str) -> (Option<String>, String) {
    for prefix in ["error ", "warning "] {
        if let Some(rest) = message.strip_prefix(prefix)
            && let Some(colon) = rest.find(": ")
            && !rest[..colon].contains(' ')
        {
            return (Some(rest[..colon].to_owned()), rest[colon + 2..].to_owned());
        }
    }
    for prefix in ["error: ", "warning: "] {
        if let Some(rest) = message.strip_prefix(prefix) {
            return (None, rest.to_owned());
        }
    }
    (None, message.to_owned())
}

// ── junit helpers ─────────────────────────────────────────────────────────

fn xml_attr(tag: &str, name: &str) -> Option<String> {
    let needle = format!(" {name}=\"");
    let at = tag.find(&needle)? + needle.len();
    let rest = &tag[at..];
    let end = rest.find('"')?;
    Some(xml_unescape(&rest[..end]))
}

fn xml_unescape(s: &str) -> String {
    s.replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&#10;", "\n")
        .replace("&amp;", "&")
}

#[cfg(test)]
#[path = "gate_rows.tests.rs"]
mod tests;
