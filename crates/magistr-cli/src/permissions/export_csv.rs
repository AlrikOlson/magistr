//! Export audit entries as RFC 4180 CSV.

use super::audit_entry::AuditEntry;

/// Render `entries` as CSV with a header row. RFC 4180: fields
/// containing commas, quotes or line breaks are double-quoted, embedded
/// quotes doubled, rows separated by CRLF.
#[must_use]
pub fn export_csv(entries: &[AuditEntry]) -> String {
    let mut out =
        String::from("timestamp,actor,tool_name,risk,allowed,reason,project,input_summary\r\n");
    for entry in entries {
        let fields = [
            entry.timestamp.to_rfc3339(),
            entry.actor.clone().unwrap_or_default(),
            entry.tool_name.clone(),
            entry.risk.to_string(),
            entry.allowed.to_string(),
            entry.reason.clone(),
            entry.project.clone().unwrap_or_default(),
            entry.input_summary.clone().unwrap_or_default(),
        ];
        let row: Vec<String> = fields.iter().map(|f| escape(f)).collect();
        out.push_str(&row.join(","));
        out.push_str("\r\n");
    }
    out
}

/// Quote a field when RFC 4180 requires it, doubling embedded quotes.
fn escape(field: &str) -> String {
    if field.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

#[cfg(test)]
#[path = "export_csv.tests.rs"]
mod tests;
