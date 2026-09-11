use serde::Serialize;

/// The response versions this build negotiates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EnvelopeVersions {
    pub min: u64,
    pub latest: u64,
}
