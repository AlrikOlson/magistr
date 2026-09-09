//! A single doctor check result.

use super::check_status::CheckStatus;

/// A single doctor check result.
#[derive(Debug, Clone)]
pub struct DoctorCheck {
    /// Name of the check.
    pub name: &'static str,
    /// What was checked (human-readable).
    pub description: String,
    /// Result of the check.
    pub status: CheckStatus,
    /// Additional detail or advice.
    pub detail: Option<String>,
}

impl std::fmt::Display for DoctorCheck {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {}: {}",
            self.status.icon(),
            self.name,
            self.description
        )?;
        if let Some(detail) = &self.detail {
            write!(f, "\n  → {detail}")?;
        }
        Ok(())
    }
}
