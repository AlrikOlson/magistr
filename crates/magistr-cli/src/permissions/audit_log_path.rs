//! Resolve the full path to the audit log file.

use std::path::PathBuf;

/// Default filename for the audit log.
const AUDIT_FILENAME: &str = "permissions.log";

/// Resolve the full path to the audit log file.
pub fn audit_log_path() -> Option<PathBuf> {
    crate::cost::storage_paths::data_dir().map(|d| d.join(AUDIT_FILENAME))
}
