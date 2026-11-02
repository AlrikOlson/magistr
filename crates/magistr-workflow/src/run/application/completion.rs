//! Whether the current run's check_results let a chunk complete.

use std::collections::BTreeMap;

use crate::run::domain::check_result::CheckResult;
use crate::run::domain::check_result_settlement::CheckResultSettlement;
use crate::run::domain::goal::VerificationMode;

use super::{
    AcceptedCheckResult, CompletionEvidence, CompletionRefusal, GateContract, OutstandingGate,
    RecoveredFailure, RunUseCases, SourceFreshness, SourceObservation, StepAdmission,
};

impl RunUseCases {
    /// Decide whether the chunk scoped as `scope` may complete on the
    /// evidence of the current run. The verification step is the explicit
    /// `step` when given (it must be a step of the current run), else the
    /// instance the run's latest check_result was admitted on. Every gate the
    /// host requires, and every required verified check_result on that
    /// instance, must have a latest check_result that was applied, passed,
    /// ran its configured command, saw the source and the applicable
    /// configuration unchanged during its run, and certifies the workspace,
    /// selection policy, source bytes, external configuration and
    /// acceptance revision current now (a new owner generation of the same
    /// bytes is not a change). A
    /// focused goal intentionally omits project-wide gate contracts;
    /// its required acceptance-specific check_results still govern completion.
    /// Nothing here holds the engine lock while the source is observed.
    ///
    /// # Errors
    /// A typed [`CompletionRefusal`] naming what is missing and what would
    /// satisfy it.
    pub fn verify_completion(
        &self,
        scope: &str,
        step: Option<&str>,
    ) -> Result<CompletionEvidence, CompletionRefusal> {
        let port = self
            .evidence
            .as_ref()
            .ok_or(CompletionRefusal::NoEvidenceCapability)?;
        let (snapshot, acceptance_revision, ledger) = {
            let engine = self
                .lock()
                .map_err(|reason| CompletionRefusal::EvidenceUnavailable {
                    reason: reason.to_string(),
                })?;
            let acceptance = engine
                .acceptance_revision()
                .ok_or(CompletionRefusal::NoGoal)?;
            (
                engine.snapshot(),
                acceptance,
                engine.check_results().clone(),
            )
        };
        let goal = snapshot.goal.as_ref().ok_or(CompletionRefusal::NoGoal)?;
        if goal.scope != scope {
            return Err(CompletionRefusal::ForeignScope {
                expected: scope.to_owned(),
                actual: goal.scope.clone(),
            });
        }
        let check_results = ledger.for_run(&snapshot.run_id).map_err(|reason| {
            CompletionRefusal::EvidenceUnavailable {
                reason: reason.to_string(),
            }
        })?;
        let target: StepAdmission = match step {
            Some(step_id) => snapshot.admit_step(Some(step_id)).map_err(|reason| {
                CompletionRefusal::StepNotCurrent {
                    step_id: step_id.to_owned(),
                    reason,
                }
            })?,
            None => check_results
                .iter()
                .rev()
                .find(|check_result| check_result.settlement.is_some())
                .map(|check_result| check_result.target.clone())
                .ok_or_else(|| CompletionRefusal::NoCheckResults {
                    run_id: snapshot.run_id.clone(),
                })?,
        };
        let contracts = match goal.verification {
            VerificationMode::Full => {
                port.required_gates()
                    .map_err(|reason| CompletionRefusal::EvidenceUnavailable {
                        reason: reason.to_string(),
                    })?
            }
            VerificationMode::Focused => Vec::new(),
        };
        let current = port
            .observe()
            .map_err(|reason| CompletionRefusal::EvidenceUnavailable {
                reason: reason.to_string(),
            })?
            .observation();
        let by_gate = attempts_by_gate(&check_results, &target.step_instance);

        let mut evidence = CompletionEvidence {
            run_id: snapshot.run_id.clone(),
            step_id: target.step_id.clone(),
            step_instance: target.step_instance.clone(),
            source_revision: current.revision.clone(),
            config_digest: current.config_digest.clone(),
            acceptance_revision: acceptance_revision.clone(),
            accepted: Vec::new(),
            recovered: Vec::new(),
            outstanding: Vec::new(),
        };
        let mut observations = BTreeMap::new();
        let mut judged: Vec<&str> = Vec::new();
        for contract in &contracts {
            judged.push(&contract.name);
            match by_gate.get(contract.name.as_str()) {
                None => evidence.outstanding.push(OutstandingGate {
                    gate: contract.name.clone(),
                    check_result_id: None,
                    reason: format!(
                        "required gate has no check_result on step `{}` in this run (command: {})",
                        target.step_id, contract.command
                    ),
                }),
                Some(attempts) => {
                    let current = observe_check_result(
                        port.as_ref(),
                        &mut observations,
                        attempts.last().expect("gate attempts"),
                    )?;
                    judge(
                        &mut evidence,
                        &contract.name,
                        attempts,
                        Some(contract),
                        &current,
                        &acceptance_revision,
                    );
                }
            }
        }
        // Required checks the caller recorded beyond the configured gates
        // govern too: a failing acceptance-specific check is not evidence.
        for (gate, attempts) in &by_gate {
            if judged.contains(gate) {
                continue;
            }
            let Some(latest) = attempts.last() else {
                continue;
            };
            if !latest.required {
                continue;
            }
            if !latest.verified() && latest.passed() {
                // A self-reported pass neither certifies nor blocks.
                continue;
            }
            let current = observe_check_result(port.as_ref(), &mut observations, latest)?;
            judge(
                &mut evidence,
                gate,
                attempts,
                None,
                &current,
                &acceptance_revision,
            );
        }
        if evidence.is_complete() {
            Ok(evidence)
        } else {
            Err(CompletionRefusal::Outstanding(Box::new(evidence)))
        }
    }
}

/// Each gate's check results on one step instance, oldest attempt first.
fn attempts_by_gate<'a>(
    check_results: &'a [CheckResult],
    step_instance: &str,
) -> BTreeMap<&'a str, Vec<&'a CheckResult>> {
    let mut by_gate: BTreeMap<&str, Vec<&CheckResult>> = BTreeMap::new();
    for check_result in check_results
        .iter()
        .filter(|check_result| check_result.target.step_instance == step_instance)
    {
        by_gate
            .entry(&check_result.gate)
            .or_default()
            .push(check_result);
    }
    for attempts in by_gate.values_mut() {
        attempts.sort_by_key(|check_result| check_result.attempt);
    }
    by_gate
}

/// Judge one gate's attempts on the verification step: the latest decides,
/// earlier failures under an accepted latest are recovered.
fn judge(
    evidence: &mut CompletionEvidence,
    gate: &str,
    attempts: &[&CheckResult],
    contract: Option<&GateContract>,
    current: &SourceObservation,
    acceptance_revision: &str,
) {
    let Some(latest) = attempts.last() else {
        return;
    };
    let refusal = reason_to_refuse(latest, contract, current, acceptance_revision);
    match refusal {
        Some(reason) => evidence.outstanding.push(OutstandingGate {
            gate: gate.to_owned(),
            check_result_id: Some(latest.id.clone()),
            reason,
        }),
        None => {
            evidence.accepted.push(AcceptedCheckResult {
                gate: gate.to_owned(),
                check_result_id: latest.id.clone(),
                attempt: latest.attempt,
                source_revision: latest.source.as_ref().map(|s| s.revision.clone()),
            });
            for earlier in attempts.iter().filter(|r| r.attempt < latest.attempt) {
                if earlier.settlement.is_some() && !earlier.passed() {
                    evidence.recovered.push(RecoveredFailure {
                        gate: gate.to_owned(),
                        check_result_id: earlier.id.clone(),
                        attempt: earlier.attempt,
                        outcome: earlier
                            .execution()
                            .map_or_else(|| "unsettled".to_owned(), |e| e.label()),
                    });
                }
            }
        }
    }
}

/// The first reason the latest check_result is not current evidence, or `None`.
/// Each identity is judged on its own and named in the reason: the
/// workspace, the scheme, the selection policy, the source bytes, the
/// external configuration and the acceptance revision. The owner
/// generation is not judged.
fn reason_to_refuse(
    latest: &CheckResult,
    contract: Option<&GateContract>,
    current: &SourceObservation,
    acceptance_revision: &str,
) -> Option<String> {
    let execution = match latest.execution() {
        None => {
            return Some(
                "the latest attempt was admitted but never settled (interrupted or still running)"
                    .into(),
            );
        }
        Some(execution) => execution,
    };
    if let Some(CheckResultSettlement::Stale { reason }) = &latest.settlement {
        return Some(format!("the latest attempt settled stale: {reason}"));
    }
    if !latest.required {
        return Some("the latest attempt was recorded as not required".into());
    }
    if !execution.verified() {
        return Some("the latest attempt was self-reported, not run by the server".into());
    }
    if !execution.passed() {
        return Some(format!(
            "the latest attempt did not pass ({})",
            execution.label()
        ));
    }
    if let Some(contract) = contract
        && latest.command.as_deref().map(str::trim) != Some(contract.command.trim())
    {
        return Some(format!(
            "the latest attempt ran `{}`, not the configured `{}`",
            latest.command.as_deref().unwrap_or("<no command>"),
            contract.command
        ));
    }
    let Some(source) = &latest.source else {
        return Some("the latest attempt observed no source".into());
    };
    match &latest.freshness {
        Some(SourceFreshness::Unchanged) => {}
        Some(SourceFreshness::Changed { paths }) => {
            return Some(format!(
                "the workspace changed while the gate ran: {}",
                paths.join(", ")
            ));
        }
        Some(SourceFreshness::Diverged { .. }) => {
            return Some("the workspace changed while the gate ran".into());
        }
        Some(SourceFreshness::Reconfigured { layers }) if layers.is_empty() => {
            return Some("the applicable configuration changed while the gate ran".into());
        }
        Some(SourceFreshness::Reconfigured { layers }) => {
            return Some(format!(
                "the applicable configuration changed while the gate ran: {}",
                layers.join(", ")
            ));
        }
        Some(SourceFreshness::Unobserved { reason }) => {
            return Some(format!(
                "the workspace could not be re-observed after the gate: {reason}"
            ));
        }
        None => return Some("the latest attempt recorded no source validation".into()),
    }
    if let Some(drift) = source.drift_to(current) {
        return Some(drift.to_string());
    }
    if latest.acceptance_revision != acceptance_revision {
        return Some("the goal's acceptance changed since the gate ran".into());
    }
    None
}

fn observe_check_result(
    port: &dyn super::EvidencePort,
    observations: &mut BTreeMap<String, SourceObservation>,
    check_result: &CheckResult,
) -> Result<SourceObservation, CompletionRefusal> {
    let inputs = check_result
        .source
        .as_ref()
        .map(|source| source.inputs.as_slice())
        .unwrap_or_default();
    let key = (&check_result.gate, &check_result.command, inputs);
    let key =
        serde_json::to_string(&key).map_err(|error| CompletionRefusal::EvidenceUnavailable {
            reason: error.to_string(),
        })?;
    if let Some(observed) = observations.get(&key) {
        return Ok(Clone::clone(observed));
    }
    let observed = port
        .observe_check(&check_result.gate, check_result.command.as_deref(), inputs)
        .map_err(|reason| CompletionRefusal::EvidenceUnavailable { reason })?
        .observation();
    observations.insert(key, observed.clone());
    Ok(observed)
}
