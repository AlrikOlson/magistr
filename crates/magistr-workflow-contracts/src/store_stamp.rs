//! Provenance of one successfully read family file, without filesystem effects.

use std::time::SystemTime;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// File identity from the same opened handle as the validated records.
/// This is provenance, not a revision or permission to skip a later read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct StoreStamp {
    pub inode: u64,
    pub modified: SystemTime,
    pub length: u64,
}
