use super::RunEngine;
use crate::infra::transaction::write_locked;
use crate::run::application::{
    CheckAdmission, CheckOutcome, CheckRequest, CheckSettlement, RunError, SourceFreshness,
    SourceObservation, StepAdmission,
};
use crate::run::broadcast::BroadcastFrame;
use crate::run::domain::{
    check::Check, check_result::CheckResult, check_result_settlement::CheckResultSettlement,
    check_result_stage::CheckResultStage,
};
use crate::run::persistence::{PersistedState, Persistence};
use chrono::Utc;
use magistr_workflow_contracts::{Observed, ReadSource, WriteError};

impl RunEngine {
    pub(crate) fn plan_step(
        &mut self,
        id: String,
        title: String,
        step_type: crate::run::domain::step::StepType,
        estimate: Option<String>,
        decision_thread: Option<String>,
    ) -> Result<Observed<()>, RunError> {
        self.add_step(id, title, step_type, estimate, decision_thread)
    }

    pub fn refresh_if_stale(&mut self) -> Result<bool, RunError> {
        self.reload_from_store()
    }

    pub fn reload_from_store(&mut self) -> Result<bool, RunError> {
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
                let error = RunError::from(error);
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
    /// its check_result. External source observation has already finished.
    pub(crate) fn admit_check_result(
        &mut self,
        target: StepAdmission,
        request: CheckRequest,
        acceptance_revision: String,
        source: Option<SourceObservation>,
    ) -> Result<Observed<CheckAdmission>, RunError> {
        let persistence = self.persistence.clone().filter(|p| p.enabled());
        let operation = |engine: &mut Self| {
            let current = engine.current_for_settlement(persistence.as_ref())?;
            if !engine.matches_target(&current.value, &target)
                || current
                    .value
                    .goal
                    .as_ref()
                    .map(|o| o.acceptance_revision())
                    .as_deref()
                    != Some(&acceptance_revision)
            {
                return Err(RunError::refused(
                    "stale_target",
                    "The selected work changed before this check could start.",
                ));
            }
            let check_result = engine
                .check_results
                .admit(&target, &request.name, |attempt| {
                    CheckResult::admitted(
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
            Ok(check_result.map(|check_result| CheckAdmission {
                target,
                request,
                check_result_id: check_result.id,
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
    ) -> Result<CheckSettlement, RunError> {
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
    ) -> Result<Observed<PersistedState>, RunError> {
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

    fn matches_target(&self, state: &PersistedState, target: &StepAdmission) -> bool {
        target.project_id == self.project_id
            && target.execution == self.binding
            && target.run_id == state.execution.run_id
            && state.execution.steps.get(&target.step_id) == Some(&target.step_instance)
    }

    fn settle_locked(
        &mut self,
        admission: &CheckAdmission,
        outcome: &CheckOutcome,
        freshness: Option<SourceFreshness>,
        persistence: Option<&Persistence>,
    ) -> Result<CheckSettlement, RunError> {
        // The canonical check_result must exist and match in full before any write.
        let check_result =
            self.check_results
                .update(&admission.check_result_id, |check_result| {
                    if check_result.target != admission.target
                        || check_result.gate != admission.request.name
                        || check_result.check_type != admission.request.check_type
                        || check_result.command != admission.request.command
                        || check_result.required != admission.request.required
                        || check_result.acceptance_revision != admission.acceptance_revision
                        || check_result.source != admission.source
                    {
                        return Err("The result does not match its admitted check.".into());
                    }
                    if let Some(recorded) = &check_result.outcome {
                        if recorded != outcome {
                            return Err(
                                "This check already has a different recorded result.".into()
                            );
                        }
                    } else {
                        let now = Utc::now();
                        check_result.duration_ms = Some(
                            chrono::DateTime::parse_from_rfc3339(&check_result.admitted_at)
                                .map(|start| {
                                    now.signed_duration_since(start).num_milliseconds().max(0)
                                        as u64
                                })
                                .unwrap_or(0),
                        );
                        check_result.recorded_at = Some(now.to_rfc3339());
                        check_result.outcome = Some(outcome.clone());
                        check_result.freshness = freshness;
                        check_result.stage = CheckResultStage::Recorded;
                    }
                    Ok(())
                })?;
        // Every subsequent refusal is a partial operation: the full result is
        // already durable and is never replaced or assigned another attempt.
        self.apply_recorded(admission, check_result, persistence)
            .map_err(RunError::after_commit)
    }

    fn apply_recorded(
        &mut self,
        admission: &CheckAdmission,
        check_result: Observed<CheckResult>,
        persistence: Option<&Persistence>,
    ) -> Result<CheckSettlement, RunError> {
        let check = check_from_check_result(&check_result.value)?;
        let current = self.current_for_settlement(persistence)?;
        let existing =
            find_linked_check(&current.value, &admission.target, &check_result.value.id).cloned();
        if check_result.value.stage == CheckResultStage::Settled {
            return Ok(
                match check_result
                    .value
                    .settlement
                    .as_ref()
                    .expect("validated terminal check_result")
                {
                    CheckResultSettlement::Applied => CheckSettlement::Applied {
                        check: Observed {
                            value: existing.unwrap_or(check),
                            source: if find_linked_check(
                                &current.value,
                                &admission.target,
                                &check_result.value.id,
                            )
                            .is_some()
                            {
                                current.source
                            } else {
                                check_result.source
                            },
                        },
                        check_result: Box::new(check_result),
                    },
                    CheckResultSettlement::Stale { .. } => CheckSettlement::Stale {
                        admission: admission.clone(),
                        outcome: check_result
                            .value
                            .outcome
                            .clone()
                            .expect("recorded outcome"),
                        check_result: Box::new(check_result),
                    },
                },
            );
        }
        let current_target = self.matches_target(&current.value, &admission.target);
        if existing.is_none() && !current_target {
            if check_result.value.stage == CheckResultStage::Applying {
                return Err(WriteError::uncertain("The work changed before this check could be confirmed. Its recorded result has been kept.").into());
            }
            let settled = self
                .check_results
                .update(&check_result.value.id, |check_result| {
                    check_result.stage = CheckResultStage::Settled;
                    check_result.settlement = Some(CheckResultSettlement::Stale {
                        reason: "The admitted work is no longer selected.".into(),
                    });
                    check_result.settled_at = Some(Utc::now().to_rfc3339());
                    Ok(())
                })?;
            return Ok(CheckSettlement::Stale {
                admission: admission.clone(),
                outcome: settled.value.outcome.clone().expect("recorded outcome"),
                check_result: Box::new(settled),
            });
        }
        let check_observed = if let Some(existing) = existing {
            Observed {
                value: existing,
                source: current.source,
            }
        } else {
            self.check_results
                .update(&check_result.value.id, |check_result| {
                    check_result.stage = CheckResultStage::Applying;
                    Ok(())
                })?;
            let mut candidate = current.value;
            let step = candidate
                .steps
                .iter_mut()
                .find(|step| step.id == admission.target.step_id)
                .expect("validated step");
            step.checks.push(check.clone());
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
        let settled = self
            .check_results
            .update(&check_result.value.id, |check_result| {
                check_result.stage = CheckResultStage::Settled;
                check_result.settlement = Some(CheckResultSettlement::Applied);
                check_result.settled_at = Some(Utc::now().to_rfc3339());
                Ok(())
            })?;
        if current_target {
            self.broadcast(BroadcastFrame::CheckRecorded {
                step_id: admission.target.step_id.clone(),
                check: check_observed.value.clone(),
            });
        }
        Ok(CheckSettlement::Applied {
            check: check_observed,
            check_result: Box::new(settled),
        })
    }
}

fn check_from_check_result(check_result: &CheckResult) -> Result<Check, RunError> {
    let outcome = check_result
        .outcome
        .as_ref()
        .ok_or("This check has no recorded result.")?;
    Ok(Check {
        check_result_id: Some(check_result.id.clone()),
        check_type: check_result.check_type.clone(),
        name: check_result.gate.clone(),
        passed: outcome.passed,
        details: outcome.details.clone(),
        required: check_result.required,
        verified: outcome.verified,
        command: outcome.command.clone(),
        exit_code: outcome.exit_code,
        report: outcome.report.clone(),
        results: outcome.results.clone(),
        timestamp: check_result
            .recorded_at
            .clone()
            .ok_or("This check has no result timestamp.")?,
    })
}

fn find_linked_check<'a>(
    state: &'a PersistedState,
    target: &StepAdmission,
    check_result_id: &str,
) -> Option<&'a Check> {
    let steps = if state.execution.run_id == target.run_id
        && state.execution.steps.get(&target.step_id) == Some(&target.step_instance)
    {
        &state.steps
    } else {
        &state
            .saved_cycles
            .iter()
            .find(|cycle| {
                cycle.execution.run_id == target.run_id
                    && cycle.execution.steps.get(&target.step_id) == Some(&target.step_instance)
            })?
            .steps
    };
    steps
        .iter()
        .find(|step| step.id == target.step_id)?
        .checks
        .iter()
        .find(|check| check.check_result_id.as_deref() == Some(check_result_id))
}
