/// Opaque owner-managed key material; deliberately omits secret-bearing Debug.
#[derive(PartialEq, Eq)]
pub struct WorkspaceKey(pub(super) [u8; 32]);

impl WorkspaceKey {
    /// Key bytes for the owner's confirmation authenticator.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}
