use super::{
    CheckAdmission, CheckExecution, CheckOutcome, CheckRequest, CheckSettlement, ObservationDrift,
    ShipError, ShipUseCases, SourceFreshness, TaskAdmission,
};
use crate::ship::domain::check::Check;
use magistr_workflow_contracts::Observed;

impl ShipUseCases {
    pub fn compact_receipts(
        &self,
    ) -> Result<crate::ship::receipt_compaction::ReceiptCompaction, ShipError> {
        self.lock()?.compact_receipts(|| {
            self.evidence.as_ref().map_or_else(
                || Ok(std::collections::BTreeSet::new()),
                |port| port.retained_receipts(),
            )
        })
    }

    pub fn admit_task(&self, task_id: Option<&str>) -> Result<TaskAdmission, ShipError> {
        self.snapshot()?.admit_task(task_id).map_err(Into::into)
    }

    pub fn admit_check(
        &self,
        mut request: CheckRequest,
    ) -> Result<Observed<CheckAdmission>, ShipError> {
        if request.command.is_none() && request.passed.is_none() {
            return Err(ShipError::invalid_args(
                "Provide a command or a check result.",
            ));
        }
        let (target, acceptance_revision) = {
            let engine = self.lock()?;
            let target = engine.snapshot().admit_task(request.task_id.as_deref())?;
            let revision = engine
                .acceptance_revision()
                .ok_or("Set an objective before running a check.")?;
            (target, revision)
        };
        let schedule = self
            .evidence
            .as_ref()
            .map(|port| port.check_schedule(&request.name, request.command.as_deref()))
            .transpose()
            .map_err(|reason| {
                ShipError::refused(
                    "source_unavailable",
                    format!("The source could not be checked: {reason}"),
                )
            })?
            .unwrap_or_default();
        request.options.max_parallel = Some(schedule.max_parallel);
        request.options.resources = schedule.resources;
        let default_timeout = self
            .evidence
            .as_ref()
            .map(|port| port.check_timeout_secs(&request.name, request.command.as_deref()))
            .transpose()
            .map_err(ShipError::from)?
            .unwrap_or(super::CheckOptions::DEFAULT_TIMEOUT_SECS);
        let timeout = request.options.timeout_secs.get_or_insert(default_timeout);
        if *timeout == 0 || *timeout > super::CheckOptions::MAX_TIMEOUT_SECS {
            return Err(ShipError::invalid_args(format!(
                "Check timeout must be between 1 and {} seconds.",
                super::CheckOptions::MAX_TIMEOUT_SECS
            )));
        }
        let witness = self
            .evidence
            .as_ref()
            .map(|port| {
                port.observe_check(
                    &request.name,
                    request.command.as_deref(),
                    &request.options.inputs,
                )
            })
            .transpose()
            .map_err(|reason| {
                ShipError::refused(
                    "source_unavailable",
                    format!("The source could not be checked: {reason}"),
                )
            })?;
        let source = witness.as_ref().map(|witness| witness.observation());
        // Acquire the only fallible local resource before committing admission.
        let mut witnesses = self
            .witnesses
            .lock()
            .map_err(|_| ShipError::from("The source check could not be retained."))?;
        let admitted = self
            .lock()?
            .admit_receipt(target, request, acceptance_revision, source)?;
        if let Some(witness) = witness {
            witnesses.insert(
                admitted.value.receipt_id.clone(),
                std::sync::Arc::from(witness),
            );
        }
        Ok(admitted)
    }

    pub fn settle_check(
        &self,
        admission: CheckAdmission,
        outcome: CheckOutcome,
    ) -> Result<CheckSettlement, ShipError> {
        self.validate_reused_outcome(&admission, &outcome)?;
        let witness = self
            .witnesses
            .lock()
            .map_err(|_| ShipError::from("The source check could not be read."))?
            .get(&admission.receipt_id)
            .cloned();
        let freshness = match (witness, &self.evidence, &admission.source) {
            (Some(witness), _, _) => Some(
                witness
                    .validate()
                    .unwrap_or_else(|reason| SourceFreshness::Unobserved { reason }),
            ),
            // Another process admitted the check: only the digests can be
            // compared, each identity on its own — a new owner generation
            // of the same bytes is not a change.
            (None, Some(port), Some(before)) => Some(
                match port.observe_check(
                    &admission.request.name,
                    admission.request.command.as_deref(),
                    &admission.request.options.inputs,
                ) {
                    Ok(now) => match before.drift_to(&now.observation()) {
                        None => SourceFreshness::Unchanged,
                        Some(ObservationDrift::Configuration { .. }) => {
                            SourceFreshness::Reconfigured { layers: Vec::new() }
                        }
                        Some(ObservationDrift::Source { before, after }) => {
                            SourceFreshness::Diverged { before, after }
                        }
                        Some(_) => SourceFreshness::Diverged {
                            before: before.revision.clone(),
                            after: now.observation().revision,
                        },
                    },
                    Err(reason) => SourceFreshness::Unobserved { reason },
                },
            ),
            _ => None,
        };

        let result = self
            .lock()?
            .settle_admitted_check(admission.clone(), outcome, freshness);
        if result.is_ok() {
            // Retention cleanup cannot revoke a committed result.
            if let Ok(mut witnesses) = self.witnesses.lock() {
                witnesses.remove(&admission.receipt_id);
            }
        }
        result
    }

    pub fn record_check(
        &self,
        request: CheckRequest,
        outcome: CheckOutcome,
    ) -> Result<Observed<Check>, ShipError> {
        self.lock()?.record_check_full(
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
    }

    #[must_use]
    pub fn self_reported(request: &CheckRequest) -> CheckExecution {
        CheckExecution::SelfReported {
            passed: request.passed.unwrap_or(false),
        }
    }
}
