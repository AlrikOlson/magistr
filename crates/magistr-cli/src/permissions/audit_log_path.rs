//! Resolve the full path to the audit log file.

use std::path::PathBuf;

use crate::config::inputs::ProjectInputs;

/// Default filename for the audit log.
const AUDIT_FILENAME: &str = "permissions.log";

/// The audit log, `<data_dir>/permissions.log`, over the captured data
/// directory; `None` when this machine has nowhere to keep one.
#[must_use]
pub fn audit_log_path(inputs: &ProjectInputs) -> Option<PathBuf> {
    inputs.data_dir().map(|dir| dir.join(AUDIT_FILENAME))
}
