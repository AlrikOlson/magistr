use chrono::Utc;

use crate::ship::domain::check::Check;
use crate::ship::domain::check_receipt::CheckReceipt;

use super::{
    CheckAdmission, CheckExecution, CheckOutcome, CheckRequest, CheckSettlement, ShipError,
    ShipUseCases, SourceFreshness, TaskAdmission,
};

impl ShipUseCases {
    pub fn admit_task(&self, task_id: Option<&str>) -> Result<TaskAdmission, String> {
        self.snapshot()?.admit_task(task_id)
    }

    /// Admit a check before its command runs: the concrete task instance,
    /// the acceptance revision now current, and — when the host composed an
    /// evidence capability — the source as observed now. The engine lock is
    /// held only to name the target; the observation and the receipt's
    /// publication happen outside it.
    ///
    /// # Errors
    /// `passed` and `command` both absent; no task to admit on; the source
    /// cannot be observed; the ledger cannot publish the receipt.
    pub fn admit_check(&self, request: CheckRequest) -> Result<CheckAdmission, ShipError> {
        if request.command.is_none() && request.passed.is_none() {
            return Err(ShipError::invalid_args(
                "`passed` is required when no `command` is given (or pass a `command` to run and verify the check).",
            ));
        }
        let (target, acceptance_revision, ledger) = {
            let engine = self.lock()?;
            let target = engine.snapshot().admit_task(request.task_id.as_deref())?;
            let revision = engine
                .acceptance_revision()
                .ok_or_else(|| "no objective is set; ship_set_objective first".to_owned())?;
            (target, revision, engine.receipts().clone())
        };
        let witness = self
            .evidence
            .as_ref()
            .map(|port| port.observe())
            .transpose()
            .map_err(|reason| ShipError::from(format!("source observation refused: {reason}")))?;
        let source = witness.as_ref().map(|witness| witness.observation());
        let receipt = ledger.admit(&target, &request.name, |attempt| {
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
        if let Some(witness) = witness {
            self.witnesses
                .lock()
                .map_err(|_| ShipError::from("witness table poisoned".to_owned()))?
                .insert(receipt.id.clone(), witness);
        }
        Ok(CheckAdmission {
            target,
            request,
            receipt_id: receipt.id,
            acceptance_revision,
            source,
        })
    }

    /// Settle a finished command against its admission: the source is
    /// re-observed first, outside every lock, then the workflow decides
    /// whether the admitted task instance is still current and records the
    /// receipt either way.
    ///
    /// # Errors
    /// The engine lock is poisoned or the ledger cannot be written.
    pub fn settle_check(
        &self,
        admission: CheckAdmission,
        outcome: CheckOutcome,
    ) -> Result<CheckSettlement, String> {
        let witness = self
            .witnesses
            .lock()
            .map_err(|_| "witness table poisoned".to_owned())?
            .remove(&admission.receipt_id);
        let freshness = match (witness, &self.evidence, &admission.source) {
            (Some(witness), _, _) => Some(
                witness
                    .validate()
                    .unwrap_or_else(|reason| SourceFreshness::Unobserved { reason }),
            ),
            (None, Some(port), Some(before)) => Some(match port.observe() {
                Ok(now) => {
                    let after = now.observation().revision;
                    if after == before.revision {
                        SourceFreshness::Unchanged
                    } else {
                        SourceFreshness::Diverged {
                            before: before.revision.clone(),
                            after,
                        }
                    }
                }
                Err(reason) => SourceFreshness::Unobserved { reason },
            }),
            _ => None,
        };
        self.lock()?
            .settle_admitted_check(admission, outcome, freshness)
    }

    /// Immediate recording for a host whose command has already completed.
    /// Hosts that may span task/run changes should retain `admit_check` instead.
    pub fn record_check(
        &self,
        request: CheckRequest,
        outcome: CheckOutcome,
    ) -> Result<Check, String> {
        self.lock()?
            .record_check_full(
                request.task_id.as_deref(),
                request.check_type,
                request.name,
                outcome.passed,
                outcome.details,
                request.required,
                outcome.verified,
                outcome.command,
                outcome.exit_code,
                outcome.report,
                outcome.results,
            )
            .cloned()
    }

    /// The execution a self-reported request stands for.
    #[must_use]
    pub fn self_reported(request: &CheckRequest) -> CheckExecution {
        CheckExecution::SelfReported {
            passed: request.passed.unwrap_or(false),
        }
    }
}
