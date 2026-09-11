use std::sync::LazyLock;

use regex::Regex;

use super::run_parse;

static JSON_MESSAGES: LazyLock<Regex> =
    LazyLock::new(|| run_parse::regex(r"--message-format(?:=|\s+)json"));

/// The cargo subcommand of a declared line: `cargo +nightly build …` → build.
pub(super) fn cargo_subcommand(command_line: &str) -> Option<String> {
    let tokens = run_parse::tokens_of(command_line);
    let at = tokens.iter().position(|token| token == "cargo")?;
    tokens
        .into_iter()
        .skip(at + 1)
        .find(|token| !token.starts_with('+') && !token.starts_with('-'))
}

pub(super) fn uses_json_messages(command_line: &str) -> bool {
    JSON_MESSAGES.is_match(command_line)
}
