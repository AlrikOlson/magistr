use alloc::string::String;
use serde::{Deserialize, Serialize};

/// Application-issued address of a resident session in an admitted working copy.
/// Provider conversation identifiers and workflow runs have separate lifetimes.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SessionAddress {
    pub project: String,
    pub workspace: String,
    pub session: String,
}
