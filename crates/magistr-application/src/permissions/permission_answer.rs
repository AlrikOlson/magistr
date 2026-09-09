use alloc::string::String;

/// The reader's answer, preserving the exact option offered by the provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionAnswer {
    /// Select an offered option; its allow/reject meaning is resolved from the
    /// pending request by the adapter, never supplied independently by the caller.
    Select { option_id: String },
    /// Dismiss the pending request without granting permission.
    Cancel,
}
