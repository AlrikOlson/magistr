//! Retention policy for automatic session cleanup.

/// Retention policy for automatic session cleanup.
#[derive(Debug, Clone, Copy)]
pub struct RetentionPolicy {
    /// Maximum age in days before a session is purged.
    pub max_age_days: u32,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self { max_age_days: 30 }
    }
}

#[cfg(test)]
#[path = "retention_policy.tests.rs"]
mod tests;
