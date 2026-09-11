/// An admitted source anchor for preserving pre-existing move/copy diagnostics.
/// Caller-supplied bytes alone cannot establish that a broken baseline existed.
#[derive(Clone, Debug)]
pub struct BaselineSource {
    pub path: String,
    pub expected_hash: String,
}
