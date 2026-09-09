//! One model a provider offers.

/// A model the provider can drive, as the client's picker shows it. The first
/// choice a provider returns from [`Provider::models`](crate::Provider::models)
/// is the one a new session starts on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelChoice {
    /// The stable identifier the client selects by (`session/set_config_option`).
    pub id: String,
    /// The human-readable name the picker shows.
    pub label: String,
}

impl ModelChoice {
    /// A choice from its id and label.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}
