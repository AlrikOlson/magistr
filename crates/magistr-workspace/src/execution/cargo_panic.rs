use std::sync::LazyLock;

use regex::Regex;

use super::run_parse;

/// rustc ≥ 1.73: `panicked at <path>:<line>:<col>:` with the message on the
/// same line or the next; older: `panicked at '<message>', <path>:<line>:<col>`.
static PANIC_NEW: LazyLock<Regex> =
    LazyLock::new(|| run_parse::regex(r"panicked at (.+?):(\d+):(\d+):?\s*(.*)$"));
static PANIC_OLD: LazyLock<Regex> =
    LazyLock::new(|| run_parse::regex(r"panicked at '(.*)', (.+?):(\d+):(\d+)\s*$"));
static NOISE: LazyLock<Regex> = LazyLock::new(|| {
    run_parse::regex(r"^note: (?:run with `RUST_BACKTRACE|Some details are omitted)")
});
static FRAME: LazyLock<Regex> = LazyLock::new(|| run_parse::regex(r"^(\d+): (.+)$"));
static FRAME_AT: LazyLock<Regex> =
    LazyLock::new(|| run_parse::regex(r"^at (.+?)(?::(\d+))?(?::(\d+))?$"));
static FOREIGN: LazyLock<Regex> = LazyLock::new(|| {
    run_parse::regex(
        r"[\\/]\.rustup[\\/]|[\\/]toolchains[\\/]|[\\/]\.cargo[\\/]registry[\\/]|[\\/]\.cargo[\\/]git[\\/]",
    )
});

/// The quartet's file/assertion/trace lifted from one test's output block.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct PanicDetails {
    pub(super) file: Option<String>,
    pub(super) assertion: String,
    pub(super) trace: String,
}

fn is_noise(line: &str) -> bool {
    NOISE.is_match(line) || line == "stack backtrace:"
}

/// Frames the workspace did not write: the standard library and toolchains.
fn is_foreign_frame(path: &str) -> bool {
    path.starts_with("/rustc/") || FOREIGN.is_match(path)
}

/// Keep only the frames whose source lives in the workspace; each kept frame
/// is one `symbol at path:line` line.
pub(super) fn trim_backtrace(lines: &[String]) -> Vec<String> {
    let mut kept = Vec::new();
    let mut pending_symbol: Option<String> = None;
    for raw in lines {
        let line = raw.trim();
        if let Some(frame) = FRAME.captures(line) {
            pending_symbol = Some(frame[2].to_owned());
            continue;
        }
        if let Some(at) = FRAME_AT.captures(line)
            && let Some(symbol) = pending_symbol.take()
        {
            let path = &at[1];
            if !is_foreign_frame(path) {
                let location = match at.get(2) {
                    Some(line_number) => format!("{path}:{}", line_number.as_str()),
                    None => path.to_owned(),
                };
                kept.push(format!(
                    "{symbol} at {}",
                    run_parse::normalize_path(&location)
                ));
            }
        }
    }
    kept
}

/// Extract the quartet's file/assertion/trace from one test's output block
/// (already dedented). Without a panic line the assertion is empty and the
/// trace is the trimmed block — the pre-013 behaviour.
pub(super) fn parse_panic_block(block: &[String]) -> PanicDetails {
    let lines: Vec<String> = block
        .iter()
        .map(|line| line.trim_end().to_owned())
        .collect();
    for (index, raw) in lines.iter().enumerate() {
        let line = raw.trim();
        // The legacy shape first: its quoted message would otherwise be read
        // as part of the modern shape's path.
        let (file, line_no, col, mut message) = if let Some(legacy) = PANIC_OLD.captures(line) {
            (
                legacy[2].to_owned(),
                legacy[3].to_owned(),
                legacy[4].to_owned(),
                legacy[1].to_owned(),
            )
        } else if let Some(modern) = PANIC_NEW.captures(line) {
            (
                modern[1].to_owned(),
                modern[2].to_owned(),
                modern[3].to_owned(),
                modern[4].trim().to_owned(),
            )
        } else {
            continue;
        };
        let mut continuation = Vec::new();
        let mut next = index + 1;
        // The message follows on the next non-empty line when the panic line
        // ended with the colon (rustc ≥ 1.73).
        if message.is_empty() {
            while next < lines.len() && lines[next].trim().is_empty() {
                next += 1;
            }
            let candidate = lines.get(next).map(|line| line.trim()).unwrap_or_default();
            if !candidate.is_empty() && !is_noise(candidate) {
                message = candidate.to_owned();
                next += 1;
            }
        }
        // Continuation: `left: …` / `right: …` and similar lines until a
        // blank, a note, or the backtrace.
        while next < lines.len() {
            let candidate = lines[next].trim();
            if candidate.is_empty() || is_noise(candidate) {
                break;
            }
            continuation.push(candidate.to_owned());
            next += 1;
        }
        let frames = lines
            .iter()
            .position(|line| line.trim() == "stack backtrace:")
            .map(|at| trim_backtrace(&lines[at + 1..]))
            .unwrap_or_default();
        let normalized = run_parse::normalize_path(&file);
        let mut trace = vec![format!("{normalized}:{line_no}:{col}")];
        trace.extend(continuation);
        trace.extend(frames);
        return PanicDetails {
            file: Some(normalized),
            assertion: message,
            trace: run_parse::trim_trace(&trace),
        };
    }
    let kept: Vec<String> = lines
        .iter()
        .filter(|line| !is_noise(line.trim()))
        .cloned()
        .collect();
    PanicDetails {
        file: None,
        assertion: String::new(),
        trace: run_parse::trim_trace(&kept),
    }
}

#[cfg(test)]
#[path = "cargo_panic.tests.rs"]
mod tests;
