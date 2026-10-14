//! CSV export for cost entries.

use super::cost_entry::CostEntry;

/// Format cost entries as CSV.
///
/// Includes a header row and one row per entry.
pub fn to_csv(entries: &[CostEntry]) -> String {
    let mut out = String::from(
        "session_id,project,actor,model,timestamp,cost_usd,turns,duration_ms,input_tokens,output_tokens,cache_read_tokens,cache_write_tokens\n",
    );
    for e in entries {
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{}\n",
            csv_escape(&e.session_id),
            csv_escape(&e.project),
            csv_escape(e.actor.as_deref().unwrap_or("")),
            csv_escape(e.model.as_deref().unwrap_or("")),
            e.timestamp.to_rfc3339(),
            e.cost_usd
                .map(|cost| format!("{cost:.6}"))
                .unwrap_or_default(),
            e.turns,
            e.duration_ms,
            e.tokens.input_tokens,
            e.tokens.output_tokens,
            e.tokens.cache_read_tokens,
            e.tokens.cache_write_tokens,
        ));
    }
    out
}

/// Escape a CSV field — wrap in quotes if it contains commas, quotes, or newlines.
fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
#[path = "csv_export.tests.rs"]
mod tests;
