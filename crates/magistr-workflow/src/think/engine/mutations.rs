//! Reasoning edits and clearing share the project transaction.
use chrono::Utc;
use std::time::Instant;

use super::ReasoningServer;
use crate::think::broadcast::BroadcastFrame;
use crate::think::domain::{BranchStatus, BranchStatusChange, StepError};
use magistr_workflow_contracts::Observed;

impl ReasoningServer {
    pub fn revise_estimate(
        &mut self,
        new_estimate: u32,
    ) -> Result<Observed<(u32, u32)>, StepError> {
        let committed = self.transact_history(self.active_session.clone(), |candidate| {
            if new_estimate == 0 {
                return Err("estimated_total must be >= 1".to_owned().into());
            }
            let last = candidate.history.steps.last_mut().ok_or_else(|| {
                StepError::from(
                    "No steps recorded yet — record a step before revising the estimate."
                        .to_owned(),
                )
            })?;
            let previous = last.estimated_total;
            last.estimated_total = new_estimate;
            candidate.history.updated_at = Some(Utc::now().to_rfc3339());
            Ok(((previous, new_estimate), candidate.history_with_branches()))
        })?;
        if let Some(b) = &self.broadcaster {
            b.emit(BroadcastFrame::EstimateRevised {
                session_id: self.active_session.clone(),
                old: committed.value.0,
                new: committed.value.1,
                reason: None,
            });
        }
        Ok(committed)
    }

    pub fn pin_step(
        &mut self,
        step_number: u32,
        pinned: bool,
    ) -> Result<Observed<bool>, StepError> {
        let committed = self.transact_history(self.active_session.clone(), |candidate| {
            let &index = candidate.step_index.get(&step_number).ok_or_else(|| {
                StepError::from(format!(
                    "No step #{step_number} found in the current trace."
                ))
            })?;
            let previous = candidate.history.steps[index].pinned.unwrap_or(false);
            candidate.history.steps[index].pinned = pinned.then_some(true);
            candidate.history.updated_at = Some(Utc::now().to_rfc3339());
            Ok((previous, candidate.history_with_branches()))
        })?;
        if let Some(b) = &self.broadcaster {
            b.emit(BroadcastFrame::PinChanged {
                session_id: self.active_session.clone(),
                step_number,
                pinned,
            });
        }
        Ok(committed)
    }

    pub fn set_branch_status(
        &mut self,
        branch_id: &str,
        status: &str,
        merged_into: Option<u32>,
    ) -> Result<Observed<BranchStatusChange>, StepError> {
        let new_status = match status.trim().to_ascii_lowercase().as_str() {
            "active" => BranchStatus::Active,
            "merged" => BranchStatus::Merged,
            "abandoned" => BranchStatus::Abandoned,
            other => {
                return Err(format!(
                    "Unknown status \"{other}\". Use active, merged, or abandoned."
                )
                .into());
            }
        };
        let committed = self.transact_history(self.active_session.clone(), |candidate| {
            if let Some(number) = merged_into {
                if !candidate.step_numbers.contains(&number) {
                    return Err(format!(
                        "merged_into refers to step #{number}, which does not exist."
                    )
                    .into());
                }
            }
            let branch = candidate
                .branches
                .get_mut(branch_id)
                .ok_or_else(|| StepError::from(format!("Unknown branch_id \"{branch_id}\".")))?;
            let previous = status_name(&branch.status);
            branch.status = new_status;
            if matches!(new_status, BranchStatus::Merged) {
                if let Some(number) = merged_into {
                    branch.merged_into = Some(number);
                }
            } else {
                branch.merged_into = None;
            }
            let change = BranchStatusChange {
                previous: previous.into(),
                current: status_name(&new_status).into(),
                merged_into: branch.merged_into,
            };
            candidate.history.updated_at = Some(Utc::now().to_rfc3339());
            Ok((change, candidate.history_with_branches()))
        })?;
        if let Some(b) = &self.broadcaster {
            b.emit(BroadcastFrame::BranchStatusChanged {
                session_id: self.active_session.clone(),
                branch_id: branch_id.to_owned(),
                status: new_status,
                merged_into: self
                    .branches
                    .get(branch_id)
                    .and_then(|branch| branch.merged_into),
            });
        }
        Ok(committed)
    }

    /// Clear every history in the project in one commit; keep its lock inode.
    pub fn clear_history(&mut self) -> Result<Observed<()>, StepError> {
        let committed = self.transact_project(None, |project| {
            project.histories.clear();
            Ok(())
        })?;
        self.steps_since_cleanup = 0;
        self.start_time = Instant::now();
        if let Some(b) = &self.broadcaster {
            b.emit(BroadcastFrame::Cleared);
        }
        Ok(committed)
    }
}

fn status_name(status: &BranchStatus) -> &'static str {
    match status {
        BranchStatus::Active => "active",
        BranchStatus::Merged => "merged",
        BranchStatus::Abandoned => "abandoned",
    }
}
