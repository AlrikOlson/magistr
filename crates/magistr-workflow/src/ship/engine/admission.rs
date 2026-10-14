use super::ShipEngine;
use crate::infra::transaction::write_locked;
use crate::ship::application::{
    CheckAdmission, CheckOutcome, CheckRequest, CheckSettlement, ShipError, SourceFreshness,
    SourceObservation, TaskAdmission,
};
use crate::ship::broadcast::BroadcastFrame;
use crate::ship::domain::{
    check::Check, check_receipt::CheckReceipt, receipt_settlement::ReceiptSettlement,
    receipt_stage::ReceiptStage,
};
use crate::ship::persistence::{PersistedState, Persistence};
use chrono::Utc;
use magistr_workflow_contracts::{Observed, ReadSource, WriteError};

impl ShipEngine {
    pub(crate) fn plan_task(
        &mut self,
        id: String,
        title: String,
        task_type: crate::ship::domain::task::TaskType,
        estimate: Option<String>,
        think_branch: Option<String>,
    ) -> Result<Observed<()>, ShipError> {
        self.add_task(id, title, task_type, estimate, think_branch)
    }

    pub fn refresh_if_stale(&mut self) -> Result<bool, ShipError> {
        self.reload_from_store()
    }

    pub fn reload_from_store(&mut self) -> Result<bool, ShipError> {
        let Some(persistence) = self.persistence.as_ref().filter(|p| p.enabled()) else {
            self.observation.record(Ok(Observed {
                value: (),
                source: ReadSource::Memory,
            }));
            return Ok(false);
        };
        let read = match persistence.current_stamped(&self.project_id) {
            Ok(read) => read,
            Err(error) => {
                let error = ShipError::from(error);
                self.failed(&error);
                return Err(error);
            }
        };
        let changed = self.observation.current().map(|o| o.source).ok() != Some(read.source);
        let state = read.value.unwrap_or_else(|| {
            if self
                .observation
                .last_successful()
                .is_some_and(|o| o.source == ReadSource::Missing)
            {
                self.persisted_state()
            } else {
                PersistedState::empty(self.project_id.clone())
            }
        });
        self.install(state, read.source);
        Ok(changed)
    }

    /// The execution lock pins the exact admission while the ledger publishes
    /// its receipt. External source observation has already finished.
    pub(crate) fn admit_receipt(
        &mut self,
        target: TaskAdmission,
        request: CheckRequest,
        acceptance_revision: String,
        source: Option<SourceObservation>,
    ) -> Result<Observed<CheckAdmission>, ShipError> {
        let persistence = self.persistence.clone().filter(|p| p.enabled());
        let operation = |engine: &mut Self| {
            let current = engine.current_for_settlement(persistence.as_ref())?;
            if !engine.matches_target(&current.value, &target)
                || current
                    .value
                    .objective
                    .as_ref()
                    .map(|o| o.acceptance_revision())
                    .as_deref()
                    != Some(&acceptance_revision)
            {
                return Err(ShipError::refused(
                    "stale_target",
                    "The selected work changed before this check could start.",
                ));
            }
            let receipt = engine.receipts.admit(&target, &request.name, |attempt| {
                CheckReceipt::admitted(
                    uuid::Uuid::new_v4().to_string(),
                    target.clone(),
                    attempt,
                    &request,
                    acceptance_revision.clone(),
                    source.clone(),
                    Utc::now().to_rfc3339(),
                )
            })?;
            engine.install(current.value, current.source);
            Ok(receipt.map(|receipt| CheckAdmission {
                target,
                request,
                receipt_id: receipt.id,
                acceptance_revision,
                source,
            }))
        };
        let result = if let Some(p) = &persistence {
            p.locked(&self.project_id.clone(), || operation(self))
        } else {
            operation(self)
        };
        if let Err(error) = &result {
            self.failed(error);
        }
        result
    }

    pub(crate) fn settle_admitted_check(
        &mut self,
        admission: CheckAdmission,
        outcome: CheckOutcome,
        freshness: Option<SourceFreshness>,
    ) -> Result<CheckSettlement, ShipError> {
        let persistence = self.persistence.clone().filter(|p| p.enabled());
        let result = if let Some(p) = &persistence {
            p.locked(&self.project_id.clone(), || {
                self.settle_locked(&admission, &outcome, freshness, Some(p))
            })
        } else {
            self.settle_locked(&admission, &outcome, freshness, None)
        };
        if let Err(error) = &result {
            self.failed(error);
        }
        result
    }

    fn current_for_settlement(
        &self,
        persistence: Option<&Persistence>,
    ) -> Result<Observed<PersistedState>, ShipError> {
        match persistence {
            Some(p) => {
                let read = p.current_stamped(&self.project_id)?;
                Ok(read.map(|state| {
                    state.unwrap_or_else(|| PersistedState::empty(self.project_id.clone()))
                }))
            }
            None => Ok(Observed {
                value: self.persisted_state(),
                source: ReadSource::Memory,
            }),
        }
    }

    fn matches_target(&self, state: &PersistedState, target: &TaskAdmission) -> bool {
        target.project_id == self.project_id
            && target.execution == self.binding
            && target.run_id == state.execution.run_id
            && state.execution.tasks.get(&target.task_id) == Some(&target.task_instance)
    }

    fn settle_locked(
        &mut self,
        admission: &CheckAdmission,
        outcome: &CheckOutcome,
        freshness: Option<SourceFreshness>,
        persistence: Option<&Persistence>,
    ) -> Result<CheckSettlement, ShipError> {
        // The canonical receipt must exist and match in full before any write.
        let receipt = self.receipts.update(&admission.receipt_id, |receipt| {
            if receipt.target != admission.target
                || receipt.gate != admission.request.name
                || receipt.check_type != admission.request.check_type
                || receipt.command != admission.request.command
                || receipt.required != admission.request.required
                || receipt.acceptance_revision != admission.acceptance_revision
                || receipt.source != admission.source
            {
                return Err("The result does not match its admitted check.".into());
            }
            if let Some(recorded) = &receipt.outcome {
                if recorded != outcome {
                    return Err("This check already has a different recorded result.".into());
                }
            } else {
                let now = Utc::now();
                receipt.duration_ms = Some(
                    chrono::DateTime::parse_from_rfc3339(&receipt.admitted_at)
                        .map(|start| {
                            now.signed_duration_since(start).num_milliseconds().max(0) as u64
                        })
                        .unwrap_or(0),
                );
                receipt.recorded_at = Some(now.to_rfc3339());
                receipt.outcome = Some(outcome.clone());
                receipt.freshness = freshness;
                receipt.stage = ReceiptStage::Recorded;
            }
            Ok(())
        })?;
        // Every subsequent refusal is a partial operation: the full result is
        // already durable and is never replaced or assigned another attempt.
        self.apply_recorded(admission, receipt, persistence)
            .map_err(ShipError::after_commit)
    }

    fn apply_recorded(
        &mut self,
        admission: &CheckAdmission,
        receipt: Observed<CheckReceipt>,
        persistence: Option<&Persistence>,
    ) -> Result<CheckSettlement, ShipError> {
        let check = check_from_receipt(&receipt.value)?;
        let current = self.current_for_settlement(persistence)?;
        let existing =
            find_linked_check(&current.value, &admission.target, &receipt.value.id).cloned();
        if receipt.value.stage == ReceiptStage::Settled {
            return Ok(
                match receipt
                    .value
                    .settlement
                    .as_ref()
                    .expect("validated terminal receipt")
                {
                    ReceiptSettlement::Applied => CheckSettlement::Applied {
                        check: Observed {
                            value: existing.unwrap_or(check),
                            source: if find_linked_check(
                                &current.value,
                                &admission.target,
                                &receipt.value.id,
                            )
                            .is_some()
                            {
                                current.source
                            } else {
                                receipt.source
                            },
                        },
                        receipt: Box::new(receipt),
                    },
                    ReceiptSettlement::Stale { .. } => CheckSettlement::Stale {
                        admission: admission.clone(),
                        outcome: receipt.value.outcome.clone().expect("recorded outcome"),
                        receipt: Box::new(receipt),
                    },
                },
            );
        }
        let current_target = self.matches_target(&current.value, &admission.target);
        if existing.is_none() && !current_target {
            if receipt.value.stage == ReceiptStage::Applying {
                return Err(WriteError::uncertain("The work changed before this check could be confirmed. Its recorded result has been kept.").into());
            }
            let settled = self.receipts.update(&receipt.value.id, |receipt| {
                receipt.stage = ReceiptStage::Settled;
                receipt.settlement = Some(ReceiptSettlement::Stale {
                    reason: "The admitted work is no longer selected.".into(),
                });
                receipt.settled_at = Some(Utc::now().to_rfc3339());
                Ok(())
            })?;
            return Ok(CheckSettlement::Stale {
                admission: admission.clone(),
                outcome: settled.value.outcome.clone().expect("recorded outcome"),
                receipt: Box::new(settled),
            });
        }
        let check_observed = if let Some(existing) = existing {
            Observed {
                value: existing,
                source: current.source,
            }
        } else {
            self.receipts.update(&receipt.value.id, |receipt| {
                receipt.stage = ReceiptStage::Applying;
                Ok(())
            })?;
            let mut candidate = current.value;
            let task = candidate
                .tasks
                .iter_mut()
                .find(|task| task.id == admission.target.task_id)
                .expect("validated task");
            task.checks.push(check.clone());
            let source = match persistence {
                Some(p) => {
                    ReadSource::File(write_locked(&p.path_for(&self.project_id), &candidate)?)
                }
                None => ReadSource::Memory,
            };
            self.install(candidate, source);
            Observed {
                value: check,
                source,
            }
        };
        let settled = self.receipts.update(&receipt.value.id, |receipt| {
            receipt.stage = ReceiptStage::Settled;
            receipt.settlement = Some(ReceiptSettlement::Applied);
            receipt.settled_at = Some(Utc::now().to_rfc3339());
            Ok(())
        })?;
        if current_target {
            self.broadcast(BroadcastFrame::CheckRecorded {
                task_id: admission.target.task_id.clone(),
                check: check_observed.value.clone(),
            });
        }
        Ok(CheckSettlement::Applied {
            check: check_observed,
            receipt: Box::new(settled),
        })
    }
}

fn check_from_receipt(receipt: &CheckReceipt) -> Result<Check, ShipError> {
    let outcome = receipt
        .outcome
        .as_ref()
        .ok_or("This check has no recorded result.")?;
    Ok(Check {
        receipt_id: Some(receipt.id.clone()),
        check_type: receipt.check_type.clone(),
        name: receipt.gate.clone(),
        passed: outcome.passed,
        details: outcome.details.clone(),
        required: receipt.required,
        verified: outcome.verified,
        command: outcome.command.clone(),
        exit_code: outcome.exit_code,
        report: outcome.report.clone(),
        results: outcome.results.clone(),
        timestamp: receipt
            .recorded_at
            .clone()
            .ok_or("This check has no result timestamp.")?,
    })
}

fn find_linked_check<'a>(
    state: &'a PersistedState,
    target: &TaskAdmission,
    receipt_id: &str,
) -> Option<&'a Check> {
    let tasks = if state.execution.run_id == target.run_id
        && state.execution.tasks.get(&target.task_id) == Some(&target.task_instance)
    {
        &state.tasks
    } else {
        &state
            .saved_cycles
            .iter()
            .find(|cycle| {
                cycle.execution.run_id == target.run_id
                    && cycle.execution.tasks.get(&target.task_id) == Some(&target.task_instance)
            })?
            .tasks
    };
    tasks
        .iter()
        .find(|task| task.id == target.task_id)?
        .checks
        .iter()
        .find(|check| check.receipt_id.as_deref() == Some(receipt_id))
}
