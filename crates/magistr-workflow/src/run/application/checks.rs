use super::{
    CheckAdmission, CheckExecution, CheckOutcome, CheckRequest, CheckSettlement, ObservationDrift,
    RunError, RunUseCases, SourceFreshness, StepAdmission,
};
use crate::run::domain::check::Check;
use magistr_workflow_contracts::Observed;

impl RunUseCases {
    pub fn compact_check_results(
        &self,
    ) -> Result<crate::run::check_result_compaction::CheckResultCompaction, RunError> {
        self.lock()?.compact_check_results(|| {
            self.evidence.as_ref().map_or_else(
                || Ok(std::collections::BTreeSet::new()),
                |port| port.retained_check_results(),
            )
        })
    }

    pub fn admit_step(&self, step_id: Option<&str>) -> Result<StepAdmission, RunError> {
        self.snapshot()?.admit_step(step_id).map_err(Into::into)
    }

    pub fn admit_check(
        &self,
        mut request: CheckRequest,
    ) -> Result<Observed<CheckAdmission>, RunError> {
        if request.options.reuse_only && (!request.options.reuse || request.command.is_none()) {
            return Err(RunError::invalid_args(
                "Reuse only requires a command and reuse enabled.",
            ));
        }
        if request.command.is_none() && request.passed.is_none() {
            return Err(RunError::invalid_args(
                "Provide a command or a check result.",
            ));
        }
        let (target, acceptance_revision) = {
            let engine = self.lock()?;
            let target = engine.snapshot().admit_step(request.step_id.as_deref())?;
            let revision = engine
                .acceptance_revision()
                .ok_or("Open a run before running a check.")?;
            (target, revision)
        };
        let schedule = self
            .evidence
            .as_ref()
            .map(|port| port.check_schedule(&request.name, request.command.as_deref()))
            .transpose()
            .map_err(|reason| {
                RunError::refused(
                    "source_unavailable",
                    format!("The source could not be checked: {reason}"),
                )
            })?
            .unwrap_or_default();
        request.options.max_parallel = Some(schedule.max_parallel);
        request.options.resources = schedule.resources;
        let budget = match self.evidence.as_ref() {
            Some(port) => port.check_budget(
                &request.name,
                request.command.as_deref(),
                request.options.timeout_secs,
            ),
            None => super::CheckBudget::resolve(request.options.timeout_secs, None, None),
        }
        .map_err(RunError::invalid_args)?;
        request.options.timeout_secs = Some(budget.seconds);
        request.options.timeout_origin = Some(budget.origin);
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
                RunError::refused(
                    "source_unavailable",
                    format!("The source could not be checked: {reason}"),
                )
            })?;
        let source = witness.as_ref().map(|witness| witness.observation());
        // Acquire the only fallible local resource before committing admission.
        let mut witnesses = self
            .witnesses
            .lock()
            .map_err(|_| RunError::from("The source check could not be retained."))?;
        let admitted =
            self.lock()?
                .admit_check_result(target, request, acceptance_revision, source)?;
        if let Some(witness) = witness {
            witnesses.insert(
                admitted.value.check_result_id.clone(),
                std::sync::Arc::from(witness),
            );
        }
        Ok(admitted)
    }

    pub fn settle_check(
        &self,
        admission: CheckAdmission,
        outcome: CheckOutcome,
    ) -> Result<CheckSettlement, RunError> {
        self.validate_reused_outcome(&admission, &outcome)?;
        let witness = self
            .witnesses
            .lock()
            .map_err(|_| RunError::from("The source check could not be read."))?
            .get(&admission.check_result_id)
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
                witnesses.remove(&admission.check_result_id);
            }
        }
        result
    }

    pub fn record_check(
        &self,
        request: CheckRequest,
        outcome: CheckOutcome,
    ) -> Result<Observed<Check>, RunError> {
        self.lock()?.record_check_full(
            request.step_id.as_deref(),
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
