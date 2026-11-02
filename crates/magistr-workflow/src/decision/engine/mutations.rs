//! Reasoning edits and clearing share the project transaction.
use chrono::Utc;
use std::time::Instant;

use super::ReasoningServer;
use crate::decision::broadcast::BroadcastFrame;
use crate::decision::domain::{NoteError, ThreadStatus, ThreadStatusChange};
use magistr_workflow_contracts::Observed;

impl ReasoningServer {
    pub fn revise_estimate(
        &mut self,
        new_estimate: u32,
    ) -> Result<Observed<(u32, u32)>, NoteError> {
        let committed = self.transact_history(self.active_log.clone(), |candidate| {
            if new_estimate == 0 {
                return Err("estimated_total must be >= 1".to_owned().into());
            }
            let last = candidate.history.notes.last_mut().ok_or_else(|| {
                NoteError::from(
                    "No notes recorded yet — record a note before revising the estimate."
                        .to_owned(),
                )
            })?;
            let previous = last.estimated_total;
            last.estimated_total = new_estimate;
            candidate.history.updated_at = Some(Utc::now().to_rfc3339());
            Ok(((previous, new_estimate), candidate.history_with_threads()))
        })?;
        if let Some(b) = &self.broadcaster {
            b.emit(BroadcastFrame::EstimateRevised {
                log_id: self.active_log.clone(),
                old: committed.value.0,
                new: committed.value.1,
                reason: None,
            });
        }
        Ok(committed)
    }

    pub fn pin(&mut self, note_number: u32, pinned: bool) -> Result<Observed<bool>, NoteError> {
        let committed = self.transact_history(self.active_log.clone(), |candidate| {
            let &index = candidate.note_index.get(&note_number).ok_or_else(|| {
                NoteError::from(format!("No note #{note_number} found in the current log."))
            })?;
            let previous = candidate.history.notes[index].pinned.unwrap_or(false);
            candidate.history.notes[index].pinned = pinned.then_some(true);
            candidate.history.updated_at = Some(Utc::now().to_rfc3339());
            Ok((previous, candidate.history_with_threads()))
        })?;
        if let Some(b) = &self.broadcaster {
            b.emit(BroadcastFrame::PinChanged {
                log_id: self.active_log.clone(),
                note: note_number,
                pinned,
            });
        }
        Ok(committed)
    }

    pub fn set_thread_status(
        &mut self,
        thread_id: &str,
        status: &str,
        merged_into: Option<u32>,
    ) -> Result<Observed<ThreadStatusChange>, NoteError> {
        let new_status = match status.trim().to_ascii_lowercase().as_str() {
            "open" => ThreadStatus::Open,
            "merged" => ThreadStatus::Merged,
            "abandoned" => ThreadStatus::Abandoned,
            other => {
                return Err(format!(
                    "Unknown status \"{other}\". Use active, merged, or abandoned."
                )
                .into());
            }
        };
        let committed = self.transact_history(self.active_log.clone(), |candidate| {
            if let Some(number) = merged_into {
                if !candidate.note_numbers.contains(&number) {
                    return Err(format!(
                        "merged_into refers to note #{number}, which does not exist."
                    )
                    .into());
                }
            }
            let thread = candidate
                .threads
                .get_mut(thread_id)
                .ok_or_else(|| NoteError::from(format!("Unknown thread_id \"{thread_id}\".")))?;
            let previous = status_name(&thread.status);
            thread.status = new_status;
            if matches!(new_status, ThreadStatus::Merged) {
                if let Some(number) = merged_into {
                    thread.merged_into = Some(number);
                }
            } else {
                thread.merged_into = None;
            }
            let change = ThreadStatusChange {
                previous: previous.into(),
                current: status_name(&new_status).into(),
                merged_into: thread.merged_into,
            };
            candidate.history.updated_at = Some(Utc::now().to_rfc3339());
            Ok((change, candidate.history_with_threads()))
        })?;
        if let Some(b) = &self.broadcaster {
            b.emit(BroadcastFrame::ThreadStatusChanged {
                log_id: self.active_log.clone(),
                thread_id: thread_id.to_owned(),
                status: new_status,
                merged_into: self
                    .threads
                    .get(thread_id)
                    .and_then(|thread| thread.merged_into),
            });
        }
        Ok(committed)
    }

    /// Clear every history in the project in one commit; keep its lock inode.
    pub fn clear_history(&mut self) -> Result<Observed<()>, NoteError> {
        let committed = self.transact_project(None, |project| {
            project.logs.clear();
            Ok(())
        })?;
        self.notes_since_cleanup = 0;
        self.start_time = Instant::now();
        if let Some(b) = &self.broadcaster {
            b.emit(BroadcastFrame::Cleared);
        }
        Ok(committed)
    }
}

fn status_name(status: &ThreadStatus) -> &'static str {
    match status {
        ThreadStatus::Open => "open",
        ThreadStatus::Merged => "merged",
        ThreadStatus::Abandoned => "abandoned",
    }
}
