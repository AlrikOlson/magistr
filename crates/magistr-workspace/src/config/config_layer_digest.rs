use serde::{Deserialize, Serialize};

use super::ConfigLayer;

/// One external configuration layer as observed: which layer, whether a
/// file stood at its resolved path, and the SHA-256 of that file's bytes
/// (empty when absent — an absent layer and an empty file are different
/// observations). Neither the path nor a byte of content is recorded:
/// two hosts resolving the same policy share the digest, and nothing
/// about where a policy lives or what it says leaves the process.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ConfigLayerDigest {
    pub layer: ConfigLayer,
    pub present: bool,
    pub digest: String,
}
