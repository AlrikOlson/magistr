/// A confirmation refusal; no token discloses the key or approved content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ConfirmationError {
    #[error("unsupported confirmation version; preview again using this native runtime")]
    UnsupportedVersion,
    #[error("confirmation does not authenticate this exact session, operation, request and plan")]
    Mismatch,
}
