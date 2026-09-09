/// The authorization outcome, independent of any UI or transport response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionOutcome {
    /// The request may proceed.
    Allow,
    /// The request must be refused.
    Deny,
    /// A responder must explicitly resolve the request.
    Ask,
}
