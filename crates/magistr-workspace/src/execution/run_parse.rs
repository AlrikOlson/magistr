//! Shared parsing helpers for `run` and its bundled runner families.
use std::sync::LazyLock;

use regex::Regex;

pub(super) fn regex(pattern: &str) -> Regex {
    Regex::new(pattern).expect("fixed runner parsing patterns compile")
}

static WHITESPACE_RUN: LazyLock<Regex> = LazyLock::new(|| regex(r"\s+"));

/// Program-name normalization: strip the path (both separators) and `.exe`.
pub(super) fn program_basename(token: &str) -> String {
    let base = token.rsplit(['/', '\\']).next().unwrap_or(token);
    let base = base.to_lowercase();
    base.strip_suffix(".exe").unwrap_or(&base).to_owned()
}

pub(super) fn tokens_of(command_line: &str) -> Vec<String> {
    command_line
        .split_whitespace()
        .map(program_basename)
        .collect()
}

pub(super) fn has_token(command_line: &str, name: &str) -> bool {
    tokens_of(command_line).iter().any(|token| token == name)
}

/// Trim a trace block to at most `max` lines, dropping blank padding.
pub(super) fn trim_trace_max(lines: &[String], max: usize) -> String {
    lines
        .iter()
        .map(|line| line.trim_end())
        .filter(|line| !line.trim().is_empty())
        .take(max)
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn trim_trace(lines: &[String]) -> String {
    trim_trace_max(lines, 8)
}

/// Workspace-relative paths are reported with forward slashes on every
/// platform (rustc prints the host separator).
pub(super) fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

/// `\r\n` normalized, then split on `\n` (the cargo family's line reader).
pub(super) fn split_lines(text: &str) -> Vec<String> {
    text.replace("\r\n", "\n")
        .split('\n')
        .map(str::to_owned)
        .collect()
}

/// Runs of whitespace collapsed to one space, as `replace(/\s+/g, " ")`.
pub(super) fn collapse_whitespace(text: &str) -> String {
    WHITESPACE_RUN.replace_all(text, " ").into_owned()
}

#[cfg(test)]
#[path = "run_parse.tests.rs"]
mod tests;
