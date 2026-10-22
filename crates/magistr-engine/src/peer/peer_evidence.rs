use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::PlanContext;

/// Actual UTF-8 evidence shared by both peers. Include source revisions in the text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PeerEvidence {
    pub roadmap: String,
    pub code: String,
    pub routing_history: String,
}

impl PeerEvidence {
    /// Bind exact evidence bytes into the existing consent-bearing plan context.
    /// Constraints retain their deliberate ordering and are hashed by PlanRevision.
    #[must_use]
    pub fn context(&self, constraints: Vec<String>) -> PlanContext {
        PlanContext {
            roadmap_revision: commitment("roadmap", &self.roadmap),
            code_revision: commitment("code", &self.code),
            routing_history_revision: commitment("routing_history", &self.routing_history),
            constraints,
        }
    }
}

fn commitment(kind: &str, text: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(b"magistr-peer-evidence-v1\0");
    hash.update(kind.as_bytes());
    hash.update(b"\0");
    hash.update(text.as_bytes());
    format!("sha256:{}", hex::encode(hash.finalize()))
}
