//! What the judge's session left behind, and whether it admits a verdict.

use agent_client_protocol::schema::v1::StopReason;

use super::verdict::Verdict;
use crate::agent::session_supervisor::TurnOutcome;

/// The session owner's account of the judge's session once it is gone:
/// the turn's outcome as the owner confirmed it, and how the owner's
/// cleanup settled. [`JudgeEvidence::admit`] is the one rule that lets
/// the judge's words be read as a [`Verdict`]: a terminal response the
/// provider actually sent, saying `EndTurn`, over a session whose
/// cleanup settled. The words themselves are never evidence — a
/// complete answer followed by a cancel, a limit, a lost stream or a
/// close that did not settle is *not yet*, carrying the answer only in
/// its reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JudgeEvidence {
    /// The last turn's outcome as the owner recorded it; `None` when the
    /// owner never saw the turn end.
    pub outcome: Option<TurnOutcome>,
    /// The owner's declared cleanup settlement — `terminate_session`'s
    /// answer.
    pub cleanup: Result<(), String>,
}

impl JudgeEvidence {
    /// Read `text` as a verdict, if the evidence admits it.
    ///
    /// # Errors
    ///
    /// The reason the evidence is not a verdict, with `text` on it for
    /// the record: an unsettled cleanup, a stream that failed or ended,
    /// a missing terminal response, a cancelled turn, or a limit or
    /// refusal in place of `EndTurn`.
    pub fn admit(&self, text: &str) -> Result<Verdict, String> {
        if let Err(error) = &self.cleanup {
            return Err(format!(
                "the judge's session did not settle: {error}; its answer is not evidence: {text}"
            ));
        }
        let Some(outcome) = &self.outcome else {
            return Err(format!(
                "the judge's session ended before it answered: {text}"
            ));
        };
        if let Some(error) = &outcome.error {
            return Err(format!(
                "the judge's session failed before a terminal response: {error}; its answer is not evidence: {text}"
            ));
        }
        match outcome.stop_reason {
            Some(StopReason::EndTurn) => Ok(Verdict::from_text(text)),
            Some(StopReason::Cancelled) => Err(format!(
                "the judge's turn was cancelled before a verdict; its answer is not evidence: {text}"
            )),
            Some(other) => Err(format!(
                "the judge's turn ended by {other:?}, not a verdict; its answer is not evidence: {text}"
            )),
            None => Err(format!(
                "the judge's session ended before it answered: {text}"
            )),
        }
    }
}

#[cfg(test)]
#[path = "judge_evidence.tests.rs"]
mod tests;
