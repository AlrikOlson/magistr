use serde::{Deserialize, Serialize};

/// A component we changed, including the value to restore on removal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct ManagedContent {
    pub(super) original: Option<String>,
    pub(super) generated: String,
}

impl ManagedContent {
    pub(super) fn updated(
        previous: Option<&Self>,
        original: Option<String>,
        generated: String,
    ) -> Self {
        let original = match previous {
            Some(old) if original.as_deref() == Some(&old.generated) => old.original.clone(),
            _ => original,
        };
        Self {
            original,
            generated,
        }
    }
}
