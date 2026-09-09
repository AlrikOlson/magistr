//! Append a single entry to the audit log.

use std::io::Write;
use std::path::Path;

use super::audit_entry::AuditEntry;

/// Append a single entry to the audit log.
///
/// Creates the file and parent directories if they don't exist.
pub fn append_entry(path: &Path, entry: &AuditEntry) -> crate::error::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| crate::error::MagistrError::io(parent, e))?;
    }

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| crate::error::MagistrError::io(path, e))?;

    let json = serde_json::to_string(entry)?;

    writeln!(file, "{json}").map_err(|e| crate::error::MagistrError::io(path, e))?;

    Ok(())
}
