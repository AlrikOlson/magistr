use serde::{Deserialize, Serialize};

/// The effect family an operation attempt belongs to. Each family has its own
/// authority on what happened and therefore its own reconciliation rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationKind {
    /// An attempt slot claimed by the run's own checkpoint.
    Reservation,
    /// A mutation of the native workflow store (roadmap, ship, signal, think).
    NativeMutation,
    /// A gate outcome recorded as evidence on a scoped task.
    Receipt,
    /// A change frame handed to attached surfaces.
    Publication,
    /// A provider runtime launched for an attempt.
    ProviderDispatch,
}

impl OperationKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Reservation => "reservation",
            Self::NativeMutation => "native_mutation",
            Self::Receipt => "receipt",
            Self::Publication => "publication",
            Self::ProviderDispatch => "provider_dispatch",
        }
    }
}

impl core::fmt::Display for OperationKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}
