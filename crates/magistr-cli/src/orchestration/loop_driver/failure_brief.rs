//! What a red check hands the next iteration.

use crate::utils::check::CheckRun;

/// How many failure rows one brief may name.
const MAX_ROWS: usize = 20;

/// How much of a check's output stands in when it parsed no rows at all.
const FALLBACK_TAIL_BYTES: usize = 1_500;

/// The block appended to the next iteration's prompt after a red required
/// check: the failures the check's own output named, with the locations a
/// reader can open, and the exit code that produced them.
///
/// This is the payoff of the checks keeping their output. Before it, the
/// loop pause carried one sentence — "test failed (exit 101) in 153.7s" —
/// and the next iteration started from nothing, because the run's evidence
/// had been discarded inside the job that produced it.
pub(crate) fn failure_brief(red: &[&CheckRun]) -> String {
    let mut out = String::from("## The last iteration's checks failed\n");
    out.push_str("\nThese ran after your commit. Fix what they name before doing anything else.\n");
    for check in red {
        out.push_str(&format!(
            "\n### `{}` — {} (exit {})\n\n",
            check.name,
            check.verdict.label(),
            check.exit_code
        ));
        out.push_str(&format!("Command: `{}`\n\n", check.command));
        if check.rows.is_empty() {
            // Nothing parsed: the tail is the honest substitute for rows,
            // and saying it is a tail keeps it from reading as the whole.
            out.push_str("Its output named no failures this parser recognises. The tail:\n\n");
            out.push_str("```\n");
            out.push_str(tail(&check.output, FALLBACK_TAIL_BYTES).trim_end());
            out.push_str("\n```\n");
            continue;
        }
        for row in check.rows.iter().take(MAX_ROWS) {
            match row.location() {
                Some(at) => out.push_str(&format!("- `{at}` — {}\n", row.message)),
                None => out.push_str(&format!("- {}\n", row.message)),
            }
        }
        if check.rows.len() > MAX_ROWS {
            out.push_str(&format!("- … and {} more\n", check.rows.len() - MAX_ROWS));
        }
    }
    out
}

/// The last `max_bytes` of `text`, cut on a char boundary.
fn tail(text: &str, max_bytes: usize) -> &str {
    if text.len() <= max_bytes {
        return text;
    }
    let start = text
        .char_indices()
        .rev()
        .take_while(|(i, _)| text.len() - *i <= max_bytes)
        .last()
        .map_or(text.len(), |(i, _)| i);
    &text[start..]
}

#[cfg(test)]
#[path = "failure_brief.tests.rs"]
mod tests;
