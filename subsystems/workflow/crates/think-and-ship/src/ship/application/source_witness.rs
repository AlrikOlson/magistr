use super::{SourceFreshness, SourceObservation};

/// One observation of the workspace held open by the host's source
/// capability: what it saw, and — asked later — whether the tree still
/// matches it. The witness is kept in the process that admitted the check
/// and consumed exactly once at settlement; nothing about it is durable
/// beyond the [`SourceObservation`] the receipt records.
pub trait SourceWitness: Send + Sync {
    /// The observation as taken.
    fn observation(&self) -> SourceObservation;

    /// Re-observe the same workspace under the same policy and compare.
    ///
    /// # Errors
    /// The workspace could not be observed again.
    fn validate(&self) -> Result<SourceFreshness, String>;
}
