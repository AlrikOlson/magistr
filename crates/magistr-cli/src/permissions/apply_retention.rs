//! Prune the audit log file to its configured retention.

use std::path::Path;

use chrono::Utc;

use super::audit_retention::AuditRetention;
use super::load_entries::load_entries;

/// Rewrite the log at `path` keeping only what `retention` allows.
///
/// Returns the number of entries pruned. Unlimited retention or a
/// missing file is a no-op. The rewrite goes through a sibling temp
/// file and an atomic rename; malformed lines do not survive it.
pub fn apply_retention(path: &Path, retention: &AuditRetention) -> crate::error::Result<usize> {
    if retention.is_unlimited() || !path.exists() {
        return Ok(0);
    }

    let entries = load_entries(path)?;
    let before = entries.len();
    let kept = retention.apply(entries, Utc::now());
    let pruned = before - kept.len();
    if pruned == 0 {
        return Ok(0);
    }

    let mut buf = String::new();
    for entry in &kept {
        buf.push_str(&serde_json::to_string(entry)?);
        buf.push('\n');
    }
    let tmp = path.with_extension("log.tmp");
    std::fs::write(&tmp, buf).map_err(|e| crate::error::MagistrError::io(&tmp, e))?;
    std::fs::rename(&tmp, path).map_err(|e| crate::error::MagistrError::io(path, e))?;

    Ok(pruned)
}
