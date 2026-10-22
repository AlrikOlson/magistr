//! Canonical record and its related state vocabulary; these serialize and change together.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ObjectiveStatus {
    Defined,
    Active,
    Completed,
    Abandoned,
}

pub use super::verification_mode::VerificationMode;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Objective {
    pub description: String,
    pub acceptance_criteria: Vec<String>,
    pub constraints: Vec<String>,
    pub scope: String,
    #[serde(
        default = "VerificationMode::legacy_full",
        skip_serializing_if = "VerificationMode::is_full"
    )]
    pub verification: VerificationMode,
    pub status: ObjectiveStatus,
    pub project_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
}

impl Objective {
    /// The content-addressed revision of what this objective asks for: its
    /// description, acceptance criteria, constraints and scope, each
    /// length-framed so fields cannot alias. A receipt records the revision
    /// current at admission; completion refuses evidence taken under
    /// another one, so editing the acceptance mid-flight (which keeps the
    /// run id) still invalidates the gates that were run before it.
    #[must_use]
    pub fn acceptance_revision(&self) -> String {
        let mut digest = Sha256::new();
        let mut frame = |bytes: &[u8]| {
            digest.update((bytes.len() as u64).to_le_bytes());
            digest.update(bytes);
        };
        frame(self.description.as_bytes());
        for criterion in &self.acceptance_criteria {
            frame(criterion.as_bytes());
        }
        frame(b"constraints");
        for constraint in &self.constraints {
            frame(constraint.as_bytes());
        }
        frame(self.scope.as_bytes());
        frame(match self.verification {
            VerificationMode::Full => b"full",
            VerificationMode::Focused => b"focused",
        });
        digest
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
}

#[cfg(test)]
#[path = "objective.tests.rs"]
mod tests;
