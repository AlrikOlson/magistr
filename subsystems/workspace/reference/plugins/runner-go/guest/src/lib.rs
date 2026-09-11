//! The bundled go runner adapter (chunk:m5-runner-adapter): an EXACT port of
//! the TS `goAdapter` in src/tools/run.ts onto the public
//! `workbench:runner@0.1.0` interface. The behavior contract is byte-identical
//! structured output — test/runner-plugin.test.ts diffs this guest against
//! the TS twin over golden fixtures, so any drift is a red test, not a guess.
//!
//! Parsing is hand-rolled (the regex crate needs std; this guest is no_std by
//! the think:119 recipe so its module imports NOTHING — it declares zero
//! capabilities and the sandbox's default-deny has nothing to even consider).

#![no_std]

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

#[global_allocator]
static ALLOC: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}

/// wasi-libc normally provides `memcmp`; without std nothing does.
#[unsafe(no_mangle)]
unsafe extern "C" fn memcmp(a: *const u8, b: *const u8, n: usize) -> i32 {
    let mut i = 0;
    while i < n {
        let x = unsafe { *a.add(i) };
        let y = unsafe { *b.add(i) };
        if x != y {
            return i32::from(x) - i32::from(y);
        }
        i += 1;
    }
    0
}

#[allow(warnings)]
mod bindings {
    wit_bindgen::generate!({
        path: "wit",
        world: "runner-go",
        generate_all,
        std_feature,
    });
}

use bindings::exports::workbench::runner::runner::{
    Guest as RunnerGuest, RunOutput, RunReport, StructuredFailure,
};

struct Component;

impl bindings::Guest for Component {
    /// The standard plugin-tool convention (TCK conformance): identity JSON.
    fn invoke(_payload: String) -> String {
        r#"{"runner":"go","surface":"workbench:runner@0.1.0"}"#.to_string()
    }
}

impl RunnerGuest for Component {
    /// TS: hasToken(line, "go") && hasToken(line, "test").
    fn can_parse(command: String) -> bool {
        let tokens: Vec<String> = command
            .split(char::is_whitespace)
            .filter(|t| !t.is_empty())
            .map(program_basename)
            .collect();
        tokens.iter().any(|t| t == "go") && tokens.iter().any(|t| t == "test")
    }

    fn parse_output(output: RunOutput) -> Option<RunReport> {
        let joined = format!("{}\n{}", output.stdout, output.stderr);
        let lines: Vec<&str> = joined.split('\n').collect();
        let mut failures: Vec<StructuredFailure> = Vec::new();
        let mut summary: Option<String> = None;

        for i in 0..lines.len() {
            if let Some(test_name) = match_fail_header(lines[i]) {
                let mut assertion = String::new();
                let mut file: Option<String> = None;
                let mut trace: Vec<String> = Vec::new();
                let end = core::cmp::min(i + 30, lines.len());
                for body in lines.iter().take(end).skip(i + 1) {
                    if is_block_end(body) {
                        break;
                    }
                    if let Some((at_file, rest)) = match_at_line(body) {
                        if file.is_none() {
                            file = Some(at_file);
                        }
                        if assertion.is_empty() {
                            assertion = rest.trim().to_string();
                        }
                        trace.push(body.trim().to_string());
                    }
                }
                failures.push(StructuredFailure {
                    test_name,
                    file,
                    assertion,
                    trace: trim_trace(&trace),
                });
            }
            if is_summary_line(lines[i]) {
                summary = Some(lines[i].trim().to_string());
            }
        }

        if failures.is_empty() && output.exit_code != 0 {
            return None;
        }
        Some(RunReport { failures, summary })
    }
}

/// TS programBasename: basename after `/` or `\`, lowercased, `.exe` stripped.
fn program_basename(token: &str) -> String {
    let base = token.rsplit(['/', '\\']).next().unwrap_or(token);
    let lower = base.to_lowercase();
    lower.strip_suffix(".exe").unwrap_or(&lower).to_string()
}

/// TS trimTrace(lines, 8): trimEnd each, drop blank, keep 8, join "\n".
fn trim_trace(lines: &[String]) -> String {
    let kept: Vec<&str> = lines
        .iter()
        .map(|l| l.trim_end())
        .filter(|l| !l.trim().is_empty())
        .collect();
    kept[..core::cmp::min(8, kept.len())].join("\n")
}

/// TS: /^(?:\s*)--- FAIL:\s+(\S+)\s+\(([\d.]+s)\)/ → the test name.
fn match_fail_header(line: &str) -> Option<String> {
    let rest = line.trim_start();
    let rest = rest.strip_prefix("--- FAIL:")?;
    let rest = eat_whitespace(rest, 1)?;
    let (name, rest) = take_non_whitespace(rest)?;
    let rest = eat_whitespace(rest, 1)?;
    let rest = rest.strip_prefix('(')?;
    let (duration, rest) = take_while(rest, |c| c.is_ascii_digit() || c == '.');
    if duration.is_empty() {
        return None;
    }
    let rest = rest.strip_prefix('s')?;
    rest.strip_prefix(')')?;
    Some(name.to_string())
}

/// TS: /^(?:\s*)--- (?:FAIL|PASS):/ or /^(?:FAIL|PASS|ok)\b/.
fn is_block_end(line: &str) -> bool {
    let trimmed = line.trim_start();
    if trimmed.starts_with("--- FAIL:") || trimmed.starts_with("--- PASS:") {
        return true;
    }
    for word in ["FAIL", "PASS", "ok"] {
        if let Some(rest) = line.strip_prefix(word) {
            if !rest.chars().next().is_some_and(is_word_char) {
                return true;
            }
        }
    }
    false
}

/// TS: /^\s+([^\s:]+\.go):(\d+):\s*(.*)$/ → (file, message).
fn match_at_line(line: &str) -> Option<(String, &str)> {
    let after_ws = eat_whitespace(line, 1)?;
    let (token, rest) = take_while(after_ws, |c| !c.is_whitespace() && c != ':');
    if token.is_empty() || !token.ends_with(".go") {
        return None;
    }
    let rest = rest.strip_prefix(':')?;
    let (digits, rest) = take_while(rest, |c| c.is_ascii_digit());
    if digits.is_empty() {
        return None;
    }
    let rest = rest.strip_prefix(':')?;
    let message = rest.trim_start();
    Some((token.to_string(), message))
}

/// TS: /^(?:FAIL|ok)\s+\S+\s+[\d.]+s/ — prefix match only.
fn is_summary_line(line: &str) -> bool {
    for word in ["FAIL", "ok"] {
        let Some(rest) = line.strip_prefix(word) else {
            continue;
        };
        let Some(rest) = eat_whitespace(rest, 1) else {
            continue;
        };
        let (pkg, rest) = take_non_whitespace_pair(rest);
        if pkg.is_empty() {
            continue;
        }
        let Some(rest) = eat_whitespace(rest, 1) else {
            continue;
        };
        let (duration, rest) = take_while(rest, |c| c.is_ascii_digit() || c == '.');
        if !duration.is_empty() && rest.starts_with('s') {
            return true;
        }
    }
    false
}

fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Consume at least `min` whitespace chars or answer None.
fn eat_whitespace(s: &str, min: usize) -> Option<&str> {
    let trimmed = s.trim_start();
    let eaten = s.len() - trimmed.len();
    if s[..eaten].chars().count() < min {
        return None;
    }
    Some(trimmed)
}

fn take_non_whitespace(s: &str) -> Option<(&str, &str)> {
    let (token, rest) = take_while(s, |c| !c.is_whitespace());
    if token.is_empty() {
        return None;
    }
    Some((token, rest))
}

fn take_non_whitespace_pair(s: &str) -> (&str, &str) {
    take_while(s, |c| !c.is_whitespace())
}

fn take_while(s: &str, pred: impl Fn(char) -> bool) -> (&str, &str) {
    let end = s
        .char_indices()
        .find(|(_, c)| !pred(*c))
        .map_or(s.len(), |(i, _)| i);
    (&s[..end], &s[end..])
}

bindings::export!(Component with_types_in bindings);
