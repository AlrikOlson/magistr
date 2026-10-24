//! Reuse only owner-executed, still-current evidence; every task gets its own receipt.
use super::{CheckAdmission, CheckExecution, CheckOutcome, EvidencePort, ShipError, ShipUseCases};
use crate::ship::domain::check::CheckType;
use crate::ship::domain::check_receipt::CheckReceipt;

impl ShipUseCases {
    pub fn reuse_check(
        &self,
        admission: &CheckAdmission,
    ) -> Result<Option<CheckOutcome>, ShipError> {
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
    ) -> Result<Option<CheckOutcome>, ShipError> {
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
        let ledger = self.lock()?.receipts().clone();
        let records = ledger.for_check(request, source)?;
        // Compare each candidate's actual covered inputs. A newer failure or
        // unfinished execution for those inputs must still block an older pass.
        let mut latest = None;
        for receipt in records.iter().rev() {
            if receipt.id == admission.receipt_id
                || matches!(
                    receipt.execution(),
                    Some(CheckExecution::ReuseUnavailable { .. })
                )
            {
                continue;
            }
            if matching_source(receipt, admission, port)? {
                latest = Some(receipt);
                break;
            }
        }
        let Some(latest) = latest.filter(|receipt| eligible(receipt, admission)) else {
            return Ok(None);
        };
        let original = match latest.execution() {
            Some(CheckExecution::Reused { receipt_id }) => ledger.get(receipt_id)?,
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
        let mut outcome = original.outcome.expect("eligible executed receipt");
        outcome.details = format!(
            "{}{}Reused verified receipt {}. Its command, selected inputs, configuration, toolchain and environment still match.",
            request.details,
            if request.details.is_empty() { "" } else { "\n" },
            original.id
        );
        outcome.execution = CheckExecution::Reused {
            receipt_id: original.id,
        };
        outcome.exit_code = None;
        Ok(Some(outcome))
    }

    pub(crate) fn validate_reused_outcome(
        &self,
        admission: &CheckAdmission,
        outcome: &CheckOutcome,
    ) -> Result<(), ShipError> {
        let CheckExecution::Reused { receipt_id } = &outcome.execution else {
            return Ok(());
        };
        let original = self.lock()?.receipts().get(receipt_id)?;
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
            return Err(ShipError::refused(
                "invalid_reuse",
                "The earlier check no longer supplies matching verified evidence.",
            ));
        }
        Ok(())
    }
}

fn matching_source(
    receipt: &CheckReceipt,
    admission: &CheckAdmission,
    port: &dyn EvidencePort,
) -> Result<bool, ShipError> {
    let (Some(before), Some(now)) = (&receipt.source, &admission.source) else {
        return Ok(false);
    };
    if before.execution_digest.is_empty() {
        return Ok(false);
    }
    port.inputs_match(before, now).map_err(|reason| {
        ShipError::refused(
            "source_unavailable",
            format!("The earlier check inputs could not be verified: {reason}"),
        )
    })
}

fn eligible(receipt: &CheckReceipt, admission: &CheckAdmission) -> bool {
    let request = &admission.request;
    if !receipt.is_applied()
        || !receipt.passed()
        || !receipt.verified()
        || receipt.gate != request.name
        || receipt.check_type != request.check_type
        || receipt.command != request.command
        || !matches!(receipt.freshness, Some(super::SourceFreshness::Unchanged))
    {
        return false;
    }
    // Required/optional is an acceptance decision, not an execution input.
    // Elapsed time alone cannot invalidate fully bound, unchanged local evidence.
    chrono::DateTime::parse_from_rfc3339(&receipt.admitted_at)
        .is_ok_and(|recorded| recorded <= chrono::Utc::now())
}
