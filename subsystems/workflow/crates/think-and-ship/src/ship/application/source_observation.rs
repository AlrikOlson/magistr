use serde::{Deserialize, Serialize};

/// What the host's source capability observed when a check was admitted:
/// which workspace (canonical root and the owner generation then durable),
/// under which hashing scheme and selection policy, the content-addressed
/// revision of the selected bytes, and what that observation may claim
/// (`optimistic` for a live tree, `immutable` for an isolated export). The
/// manifest itself stays with the capability; the workflow keeps only what
/// a later comparison needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceObservation {
    pub workspace_root: String,
    pub workspace_generation: Option<u64>,
    pub scheme: String,
    pub policy_digest: String,
    pub revision: String,
    pub consistency: String,
    pub captured_at_ms: u64,
}
