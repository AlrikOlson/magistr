//! Reuse only owner-executed, still-current evidence; every step gets its own check_result.
use super::{CheckAdmission, CheckExecution, CheckOutcome, EvidencePort, RunError, RunUseCases};
use crate::run::domain::check::CheckType;
use crate::run::domain::check_result::CheckResult;

impl RunUseCases {
    pub fn reuse_check(
        &self,
        admission: &CheckAdmission,
    ) -> Result<Option<CheckOutcome>, RunError> {
        let found = self.find_reusable_check(admission)?;
        Ok(found.or_else(|| {
            admission
                .request
                .options
                .reuse_only
                .then(|| CheckOutcome::reuse_unavailable(admission.request.command.clone()))
        }))
    }

    fn find_reusable_check(
        &self,
        admission: &CheckAdmission,
    ) -> Result<Option<CheckOutcome>, RunError> {
        let request = &admission.request;
        let Some(port) = self.evidence.as_deref() else {
            return Ok(None);
        };
        if !request.options.reuse
            || !matches!(
                request.check_type,
                CheckType::Test | CheckType::Lint | CheckType::Typecheck
            )
            || !port.can_reuse(request)
        {
            return Ok(None);
        }
        let Some(source) = &admission.source else {
            return Ok(None);
        };
        if source.execution_digest.is_empty() {
            return Ok(None);
        }
        let ledger = self.lock()?.check_results().clone();
        let records = ledger.for_check(request, source)?;
        // Compare each candidate's actual covered inputs. A newer failure or
        // unfinished execution for those inputs must still block an older pass.
        let mut latest = None;
        for check_result in records.iter().rev() {
            if check_result.id == admission.check_result_id
                || matches!(
                    check_result.execution(),
                    Some(CheckExecution::ReuseUnavailable { .. })
                )
            {
                continue;
            }
            if matching_source(check_result, admission, port)? {
                latest = Some(check_result);
                break;
            }
        }
        let Some(latest) = latest.filter(|check_result| eligible(check_result, admission)) else {
            return Ok(None);
        };
        let original = match latest.execution() {
            Some(CheckExecution::Reused { check_result_id }) => ledger.get(check_result_id)?,
            _ => Some(latest.clone()),
        };
        let Some(original) = original else {
            return Ok(None);
        };
        if !eligible(&original, admission)
            || !matching_source(&original, admission, port)?
            || !matches!(
                original.execution(),
                Some(CheckExecution::Exited { code: 0 })
            )
        {
            return Ok(None);
        }
        let mut outcome = original.outcome.expect("eligible executed check_result");
        outcome.details = format!(
            "{}{}Reused verified check_result {}. Its command, selected inputs, configuration, toolchain and environment still match.",
            request.details,
            if request.details.is_empty() { "" } else { "\n" },
            original.id
        );
        outcome.execution = CheckExecution::Reused {
            check_result_id: original.id,
        };
        outcome.exit_code = None;
        Ok(Some(outcome))
    }

    pub(crate) fn validate_reused_outcome(
        &self,
        admission: &CheckAdmission,
        outcome: &CheckOutcome,
    ) -> Result<(), RunError> {
        let CheckExecution::Reused { check_result_id } = &outcome.execution else {
            return Ok(());
        };
        let original = self.lock()?.check_results().get(check_result_id)?;
        let valid =
            if let (Some(port), Some(original)) = (self.evidence.as_deref(), original.as_ref()) {
                admission.request.options.reuse
                    && port.can_reuse(&admission.request)
                    && eligible(original, admission)
                    && matching_source(original, admission, port)?
                    && matches!(
                        original.execution(),
                        Some(CheckExecution::Exited { code: 0 })
                    )
            } else {
                false
            };
        if !valid {
            return Err(RunError::refused(
                "invalid_reuse",
                "The earlier check no longer supplies matching verified evidence.",
            ));
        }
        Ok(())
    }
}

fn matching_source(
    check_result: &CheckResult,
    admission: &CheckAdmission,
    port: &dyn EvidencePort,
) -> Result<bool, RunError> {
    let (Some(before), Some(now)) = (&check_result.source, &admission.source) else {
        return Ok(false);
    };
    if before.execution_digest.is_empty() {
        return Ok(false);
    }
    port.inputs_match(before, now).map_err(|reason| {
        RunError::refused(
            "source_unavailable",
            format!("The earlier check inputs could not be verified: {reason}"),
        )
    })
}

fn eligible(check_result: &CheckResult, admission: &CheckAdmission) -> bool {
    let request = &admission.request;
    if !check_result.is_applied()
        || !check_result.passed()
        || !check_result.verified()
        || check_result.gate != request.name
        || check_result.check_type != request.check_type
        || check_result.command != request.command
        || !matches!(
            check_result.freshness,
            Some(super::SourceFreshness::Unchanged)
        )
    {
        return false;
    }
    // Required/optional is an acceptance decision, not an execution input.
    // Elapsed time alone cannot invalidate fully bound, unchanged local evidence.
    chrono::DateTime::parse_from_rfc3339(&check_result.admitted_at)
        .is_ok_and(|recorded| recorded <= chrono::Utc::now())
}
