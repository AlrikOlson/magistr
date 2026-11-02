//! Note revisions — back-pointer bookkeeping when a new note revises
//! an earlier one.
//!
//! `process_note` calls this when the incoming note carries
//! `revises_note`. We refuse forward references (you can only revise
//! earlier notes), mark the original note's `revised_by` pointer, and
//! bump the per-history `revisions_count` metadata counter. A missing
//! `revises_note` target is a soft warning, not an error — the new
//! note still gets recorded, just without the back-pointer.

use crate::decision::domain::Note;

use super::core::ReasoningServer;

impl ReasoningServer {
    pub(crate) fn validate_revision(&self, note: &Note) -> Result<(), String> {
        let Some(revises) = note.revises_note else {
            return Ok(());
        };
        if !self.config.features.enable_revisions {
            return Ok(());
        }
        if revises >= note.note {
            let msg = format!(
                "Cannot revise note {revises} from note {}: can only revise earlier notes",
                note.note
            );
            tracing::warn!("{msg}");
            return Err(msg);
        }

        Ok(())
    }
}
