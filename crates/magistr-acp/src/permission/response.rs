//! The application's answer to a permission request.

/// Response to an [`AcpPermissionRequest`](super::AcpPermissionRequest).
#[derive(Debug)]
pub enum AcpPermissionResponse {
    /// The user picked an option (by id).
    Selected(String),
    /// The user dismissed the prompt.
    Cancelled,
}
