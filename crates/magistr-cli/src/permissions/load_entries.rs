//! Load all audit entries from a log file.

use std::io::BufRead;
use std::path::Path;

use super::audit_entry::AuditEntry;

/// Load all audit entries from a log file.
///
/// Returns an empty vec if the file does not exist.
/// Skips malformed lines.
pub fn load_entries(path: &Path) -> crate::error::Result<Vec<AuditEntry>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let file = std::fs::File::open(path).map_err(|e| crate::error::MagistrError::io(path, e))?;
    let reader = std::io::BufReader::new(file);
    let mut entries = Vec::new();

    for line in reader.lines() {
        let line = line.map_err(|e| crate::error::MagistrError::io(path, e))?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(entry) = serde_json::from_str::<AuditEntry>(trimmed) {
            entries.push(entry);
        }
        // Skip malformed lines silently
    }

    Ok(entries)
}
