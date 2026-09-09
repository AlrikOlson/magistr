//! The shared output budget for a gate or format command.

/// Maximum output size kept from a gate/format command (100 KB).
///
/// Larger output is truncated with a marker so it cannot overwhelm an LLM
/// context. Failure rows are always parsed from the **full** output, before
/// this runs — the diagnostic that matters can sit past the cut.
pub(crate) const MAX_OUTPUT_BYTES: usize = 100_000;

/// Truncate command output to `max_bytes`, preserving the head and tail
/// with a `[truncated]` marker in between.
///
/// Splits 80% head / 20% tail so the beginning of output (most useful for
/// diagnostics) is preserved in full while still showing the final lines.
pub(crate) fn truncate_output(output: &str, max_bytes: usize) -> String {
    if output.len() <= max_bytes {
        return output.to_string();
    }
    let total = output.len();
    let head_budget = max_bytes * 4 / 5; // 80%
    let tail_budget = max_bytes - head_budget; // 20%

    // Find char-safe split points (don't cut mid-UTF-8)
    let head_end = output
        .char_indices()
        .take_while(|(i, _)| *i <= head_budget)
        .last()
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0);
    let tail_start = output
        .char_indices()
        .rev()
        .take_while(|(i, _)| total - *i <= tail_budget)
        .last()
        .map(|(i, _)| i)
        .unwrap_or(total);

    format!(
        "{}\n\n[... truncated, {} bytes total ...]\n\n{}",
        &output[..head_end],
        total,
        &output[tail_start..]
    )
}

#[cfg(test)]
#[path = "truncate_output.tests.rs"]
mod tests;
