use chrono::Utc;

use crate::ship::application::{CheckAdmission, CheckOutcome, CheckSettlement, SourceFreshness};
use crate::ship::broadcast::BroadcastFrame;
use crate::ship::domain::check::Check;
use crate::ship::domain::check_receipt::CheckReceipt;
use crate::ship::domain::execution_identity::ExecutionIdentity;
use crate::ship::domain::receipt_settlement::ReceiptSettlement;
use crate::ship::persistence::PersistedState;

use super::ShipEngine;

impl ShipEngine {
    pub(crate) fn plan_task(
        &mut self,
        id: String,
        title: String,
        task_type: crate::ship::domain::task::TaskType,
        estimate: Option<String>,
        think_branch: Option<String>,
    ) -> Result<(), String> {
        if self.tasks.iter().any(|task| task.id == id) {
            return Err(format!("task '{id}' already exists"));
        }
        self.add_task(id, title, task_type, estimate, think_branch);
        Ok(())
    }

    /// Observe the persisted cycle before answering or acting: another
    /// admitted role may have advanced the run. A `stat` compares the
    /// file's stamp with the one this memory was read from or written to;
    /// only a moved stamp re-reads the file, which then replaces memory
    /// whole (a missing file is the cleared cycle). Answers whether it
    /// re-read.
    pub fn refresh_if_stale(&mut self) -> Result<bool, String> {
        let Some(persistence) = &self.persistence else {
            return Ok(false);
        };
        if !persistence.enabled() {
            return Ok(false);
        }
        let on_disk = persistence
            .stamp(&self.project_id)
            .map_err(|error| error.to_string())?;
        if on_disk == self.stamp {
            return Ok(false);
        }
        self.reload_from_store()
    }

    /// Re-read the persisted cycle whatever the stamp says; the engine's
    /// wiring (broadcaster, mirror, cloud, receipts) stays. Answers `true`
    /// when persistence is on.
    pub fn reload_from_store(&mut self) -> Result<bool, String> {
        let Some(persistence) = &self.persistence else {
            return Ok(false);
        };
        if !persistence.enabled() {
            return Ok(false);
        }
        let (state, stamp) = persistence
            .current_stamped(&self.project_id)
            .map_err(|error| error.to_string())?;
        self.adopt_current(state);
        self.stamp = stamp;
        Ok(true)
    }

    /// Settle an admitted check: the result lands on the admitted task
    /// instance only while that instance, run and project are still
    /// current; otherwise it stays with its admission. Either way the
    /// receipt is rewritten under its original id with the execution, the
    /// timing, the source freshness and how it was settled — a late result
    /// after an objective rollover keeps its record and never reaches the
    /// task that reused the name.
    pub(crate) fn settle_admitted_check(
        &mut self,
        admission: CheckAdmission,
        outcome: CheckOutcome,
        freshness: Option<SourceFreshness>,
    ) -> Result<CheckSettlement, String> {
        let check = Check {
            check_type: admission.request.check_type.clone(),
            name: admission.request.name.clone(),
            passed: outcome.passed,
            details: outcome.details.clone(),
            required: admission.request.required,
            verified: outcome.verified,
            command: outcome.command.clone(),
            exit_code: outcome.exit_code,
            report: outcome.report.clone(),
            results: outcome.results.clone(),
            timestamp: Utc::now().to_rfc3339(),
        };
        let stale = if admission.target.project_id != self.project_id {
            Some("the admitted project is not this project".to_owned())
        } else if let Some(persistence) = &self.persistence
            && persistence.enabled()
        {
            let (current, applied, stamp) = persistence
                .settle_check(&admission.target, &check)
                .map_err(|error| error.to_string())?;
            self.adopt_current(current);
            self.stamp = stamp;
            (!applied).then(|| self.stale_reason(&admission))
        } else if self.snapshot().contains(&admission.target) {
            let task = self
                .tasks
                .iter_mut()
                .find(|task| task.id == admission.target.task_id)
                .expect("admitted task exists");
            task.checks.push(check.clone());
            None
        } else {
            Some(self.stale_reason(&admission))
        };
        let settlement = match &stale {
            None => ReceiptSettlement::Applied,
            Some(reason) => ReceiptSettlement::Stale {
                reason: reason.clone(),
            },
        };
        let settled_at = Utc::now();
        let receipt = self.receipts.settle(
            &admission.receipt_id,
            || {
                CheckReceipt::admitted(
                    admission.receipt_id.clone(),
                    admission.target.clone(),
                    0,
                    &admission.request,
                    admission.acceptance_revision.clone(),
                    admission.source.clone(),
                    settled_at.to_rfc3339(),
                )
            },
            |receipt| {
                receipt.settled_at = Some(settled_at.to_rfc3339());
                receipt.duration_ms = chrono::DateTime::parse_from_rfc3339(&receipt.admitted_at)
                    .ok()
                    .and_then(|admitted| {
                        u64::try_from(
                            (settled_at - admitted.with_timezone(&Utc)).num_milliseconds(),
                        )
                        .ok()
                    });
                receipt.execution = Some(outcome.execution.clone());
                receipt.freshness = freshness;
                receipt.settlement = Some(settlement);
            },
        )?;
        if stale.is_some() {
            return Ok(CheckSettlement::Stale {
                admission,
                outcome,
                receipt: Box::new(receipt),
            });
        }
        self.broadcast(BroadcastFrame::CheckRecorded {
            task_id: admission.target.task_id,
            check: check.clone(),
        });
        Ok(CheckSettlement::Applied {
            check,
            receipt: Box::new(receipt),
        })
    }

    /// Why an admission no longer names a current task instance.
    fn stale_reason(&self, admission: &CheckAdmission) -> String {
        if admission.target.run_id != self.execution.run_id {
            format!(
                "run {} was superseded by run {}",
                admission.target.run_id, self.execution.run_id
            )
        } else {
            format!(
                "task '{}' instance {} is no longer current in run {}",
                admission.target.task_id, admission.target.task_instance, self.execution.run_id
            )
        }
    }

    pub(super) fn adopt_current(&mut self, current: Option<PersistedState>) {
        match current {
            Some(state) => {
                self.execution = state.execution.unwrap_or_else(|| {
                    ExecutionIdentity::legacy(&self.project_id, &state.objective, &state.tasks)
                });
                self.objective = state.objective;
                self.tasks = state.tasks;
                self.next_action_id = state.next_action_id;
            }
            None if self.objective.is_some() || !self.tasks.is_empty() => {
                self.objective = None;
                self.tasks.clear();
                self.next_action_id = 1;
                self.execution = ExecutionIdentity::fresh();
            }
            None => {}
        }
    }
}
