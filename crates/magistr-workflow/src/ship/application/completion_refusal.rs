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
    /// No ship objective is set in this project.
    NoObjective,
    /// The current objective is scoped to another chunk (or to nothing).
    ForeignScope { expected: String, actual: String },
    /// The named task is not a task of the current run.
    TaskNotCurrent { task_id: String, reason: String },
    /// Nothing was ever verified in this run, so no task can be named.
    NoReceipts { run_id: String },
    /// The evidence capability could not answer.
    EvidenceUnavailable { reason: String },
    /// At least one required gate has no current, passing receipt.
    Outstanding(Box<CompletionEvidence>),
}

impl fmt::Display for CompletionRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoEvidenceCapability => write!(
                f,
                "completion refused: no verification evidence capability is composed for this project"
            ),
            Self::NoObjective => write!(
                f,
                "completion refused: no ship objective is set — ship_set_objective scoped to this chunk, run the gates on its verification task, then complete"
            ),
            Self::ForeignScope { expected, actual } => write!(
                f,
                "completion refused: the ship objective is scoped to `{actual}`, not `{expected}` — its receipts certify another chunk"
            ),
            Self::TaskNotCurrent { task_id, reason } => write!(
                f,
                "completion refused: task `{task_id}` is not a task of the current run ({reason}) — name the verification task the gates were recorded on"
            ),
            Self::NoReceipts { run_id } => write!(
                f,
                "completion refused: no check was recorded in run {run_id} — run every required gate on the verification task first"
            ),
            Self::EvidenceUnavailable { reason } => {
                write!(f, "completion refused: evidence unavailable — {reason}")
            }
            Self::Outstanding(evidence) => {
                write!(
                    f,
                    "completion refused: {} required gate(s) lack current evidence on task `{}` (run {}):",
                    evidence.outstanding.len(),
                    evidence.task_id,
                    evidence.run_id
                )?;
                for gate in &evidence.outstanding {
                    write!(f, "\n  - {}: {}", gate.gate, gate.reason)?;
                }
                write!(
                    f,
                    "\nRe-run the outstanding gates on `{}` against the current tree and complete again.",
                    evidence.task_id
                )
            }
        }
    }
}

impl std::error::Error for CompletionRefusal {}
