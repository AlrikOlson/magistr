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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Objective {
    pub description: String,
    pub acceptance_criteria: Vec<String>,
    pub constraints: Vec<String>,
    pub scope: String,
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
        format!("{:x}", digest.finalize())
    }
}
