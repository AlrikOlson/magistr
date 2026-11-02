use std::fmt;

use serde::{Deserialize, Serialize};

use super::CompletionEvidence;

/// Why a chunk may not complete now, in the words the caller needs to act
/// on: which authority refused and what would satisfy it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "refusal", rename_all = "snake_case")]
pub enum CompletionRefusal {
    /// The host composed no evidence capability, so nothing can certify
    /// the inputs a check ran against.
    NoEvidenceCapability,
    /// No run goal is set in this project.
    NoGoal,
    /// The current goal is scoped to another chunk (or to nothing).
    ForeignScope { expected: String, actual: String },
    /// The named step is not a step of the current run.
    StepNotCurrent { step_id: String, reason: String },
    /// Nothing was ever verified in this run, so no step can be named.
    NoCheckResults { run_id: String },
    /// The evidence capability could not answer.
    EvidenceUnavailable { reason: String },
    /// At least one required gate has no current, passing check_result.
    Outstanding(Box<CompletionEvidence>),
}

impl fmt::Display for CompletionRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoEvidenceCapability => write!(
                f,
                "completion refused: no verification evidence capability is composed for this project"
            ),
            Self::NoGoal => write!(
                f,
                "completion refused: no run goal is set — run_open scoped to this chunk, run the gates on its verification step, then complete"
            ),
            Self::ForeignScope { expected, actual } => write!(
                f,
                "completion refused: the run goal is scoped to `{actual}`, not `{expected}` — its check_results certify another chunk"
            ),
            Self::StepNotCurrent { step_id, reason } => write!(
                f,
                "completion refused: step `{step_id}` is not a step of the current run ({reason}) — name the verification step the gates were recorded on"
            ),
            Self::NoCheckResults { run_id } => write!(
                f,
                "completion refused: no check was recorded in run {run_id} — run every required gate on the verification step first"
            ),
            Self::EvidenceUnavailable { reason } => {
                write!(f, "completion refused: evidence unavailable — {reason}")
            }
            Self::Outstanding(evidence) => {
                write!(
                    f,
                    "completion refused: {} required gate(s) lack current evidence on step `{}` (run {}):",
                    evidence.outstanding.len(),
                    evidence.step_id,
                    evidence.run_id
                )?;
                for gate in &evidence.outstanding {
                    write!(f, "\n  - {}: {}", gate.gate, gate.reason)?;
                }
                write!(
                    f,
                    "\nRe-run the outstanding gates on `{}` against the current tree and complete again.",
                    evidence.step_id
                )
            }
        }
    }
}

impl std::error::Error for CompletionRefusal {}
