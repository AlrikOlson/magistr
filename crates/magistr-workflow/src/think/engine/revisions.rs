//! Step revisions — back-pointer bookkeeping when a new step revises
//! an earlier one.
//!
//! `process_step` calls this when the incoming step carries
//! `revises_step`. We refuse forward references (you can only revise
//! earlier steps), mark the original step's `revised_by` pointer, and
//! bump the per-history `revisions_count` metadata counter. A missing
//! `revises_step` target is a soft warning, not an error — the new
//! step still gets recorded, just without the back-pointer.

use crate::think::domain::ThinkStep;

use super::core::ReasoningServer;

impl ReasoningServer {
    pub(crate) fn validate_revision(&self, step: &ThinkStep) -> Result<(), String> {
        let Some(revises) = step.revises_step else {
            return Ok(());
        };
        if !self.config.features.enable_revisions {
            return Ok(());
        }
        if revises >= step.step_number {
            let msg = format!(
                "Cannot revise step {revises} from step {}: can only revise earlier steps",
                step.step_number
            );
            tracing::warn!("{msg}");
            return Err(msg);
        }

        Ok(())
    }
}
