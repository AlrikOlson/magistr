//! One row of a provider's model registry.

use crate::{ModelChoice, Pricing};

/// A model an API provider offers: the id the API takes, the label the
/// picker shows, its context window and what it charges.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelSpec {
    /// The id sent on the wire and selected through the picker.
    pub id: String,
    /// The picker's label.
    pub label: String,
    /// The context window, in tokens — the `size` of every usage update.
    pub context_window: u64,
    /// What the model charges.
    pub pricing: Pricing,
}

impl ModelSpec {
    /// A registry row.
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        context_window: u64,
        pricing: Pricing,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            context_window,
            pricing,
        }
    }

    /// The row as the picker shows it.
    #[must_use]
    pub fn choice(&self) -> ModelChoice {
        ModelChoice::new(self.id.clone(), self.label.clone())
    }
}
