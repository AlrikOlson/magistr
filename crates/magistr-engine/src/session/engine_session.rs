use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{EngineError, Result};
use crate::workflow::WorkflowPhase;

use super::session_command::SessionCommand;
use super::session_cost::SessionCost;
use super::session_status::SessionStatus;

/// The core domain aggregate for an agent session.
///
/// Owns the session state machine, cost tracking, and workflow phase.
/// Does not perform IO — infrastructure adapters translate commands
/// into protocol actions and protocol messages into events.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EngineSession {
    /// Unique session identifier.
    session_id: String,
    /// Current lifecycle status.
    status: SessionStatus,
    /// Current workflow phase.
    phase: WorkflowPhase,
    /// Accumulated cost and token usage.
    cost: SessionCost,
    /// When the session was created.
    created_at: DateTime<Utc>,
    /// Message log (assistant messages, for summary generation).
    messages: Vec<String>,
}

impl EngineSession {
    /// Create a new session in the `Created` state.
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            status: SessionStatus::Created,
            phase: WorkflowPhase::Researching,
            cost: SessionCost::default(),
            created_at: Utc::now(),
            messages: Vec::new(),
        }
    }

    /// Rebuild a session from a snapshot's fields.
    ///
    /// The restore path, and the only constructor that may land a
    /// session in a terminal status: [`set_status`] refuses to move
    /// *out* of one, which is the right rule for a running session and
    /// the wrong one for reading a document that recorded a session
    /// which had already ended.
    ///
    /// [`set_status`]: EngineSession::set_status
    #[must_use]
    pub fn restored(
        session_id: impl Into<String>,
        status: SessionStatus,
        phase: WorkflowPhase,
        cost: SessionCost,
        created_at: DateTime<Utc>,
        messages: Vec<String>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            status,
            phase,
            cost,
            created_at,
            messages,
        }
    }

    /// Create a session with a specific creation time (for testing).
    #[cfg(test)]
    pub fn with_created_at(session_id: impl Into<String>, created_at: DateTime<Utc>) -> Self {
        Self {
            session_id: session_id.into(),
            status: SessionStatus::Created,
            phase: WorkflowPhase::Researching,
            cost: SessionCost::default(),
            created_at,
            messages: Vec::new(),
        }
    }

    // ── Accessors ───────────────────────────────────────────────────────

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn status(&self) -> SessionStatus {
        self.status
    }

    pub fn phase(&self) -> WorkflowPhase {
        self.phase
    }

    pub fn cost(&self) -> &SessionCost {
        &self.cost
    }

    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    pub fn messages(&self) -> &[String] {
        &self.messages
    }

    // ── State transitions ───────────────────────────────────────────────

    /// Transition to a new status.
    ///
    /// Validates the transition is legal before applying it.
    pub fn set_status(&mut self, new_status: SessionStatus) -> Result<()> {
        if self.status.is_terminal() {
            return Err(EngineError::invalid_command(
                format!("set_status({new_status})"),
                self.status,
                "session is in a terminal state",
            ));
        }
        self.status = new_status;
        Ok(())
    }

    /// Update the workflow phase.
    pub fn set_phase(&mut self, phase: WorkflowPhase) {
        self.phase = phase;
    }

    /// Record a completed turn with cost data.
    pub fn record_turn(&mut self, cost_usd: f64, input_tokens: u64, output_tokens: u64) {
        self.cost.record_turn(cost_usd, input_tokens, output_tokens);
    }

    /// Append a message to the log.
    pub fn push_message(&mut self, content: impl Into<String>) {
        self.messages.push(content.into());
    }

    // ── Command validation ──────────────────────────────────────────────

    /// Validate whether a command can be executed in the current state.
    ///
    /// Returns `Ok(())` if valid, `Err(EngineError::InvalidCommand)` otherwise.
    pub fn validate_command(&self, command: &SessionCommand) -> Result<()> {
        match command {
            SessionCommand::SendFollowup { .. } => {
                if !self.status.can_send_followup() {
                    return Err(EngineError::invalid_command(
                        command.name(),
                        self.status,
                        "can only send follow-ups when Awaiting",
                    ));
                }
            }
            SessionCommand::ApprovePermission { .. } | SessionCommand::DenyPermission { .. } => {
                if self.status.is_terminal() {
                    return Err(EngineError::invalid_command(
                        command.name(),
                        self.status,
                        "session has ended",
                    ));
                }
            }
            SessionCommand::Cancel => {
                if !self.status.can_cancel() {
                    return Err(EngineError::invalid_command(
                        command.name(),
                        self.status,
                        "can only cancel InProgress or Awaiting sessions",
                    ));
                }
            }
            SessionCommand::Abort => {
                // Abort is always allowed — it's a hard kill.
            }
            SessionCommand::SetConfig { .. } => {
                if self.status.is_terminal() {
                    return Err(EngineError::invalid_command(
                        command.name(),
                        self.status,
                        "session has ended",
                    ));
                }
            }
        }
        Ok(())
    }
}
