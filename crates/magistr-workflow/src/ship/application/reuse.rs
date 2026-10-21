//! Reuse only owner-executed, still-current evidence; every task gets its own receipt.

use super::{CheckAdmission, CheckExecution, CheckOutcome, ShipError, ShipUseCases};
use crate::ship::domain::check::CheckType;
use crate::ship::domain::check_receipt::CheckReceipt;

impl ShipUseCases {
    pub fn reuse_check(
        &self,
        admission: &CheckAdmission,
    ) -> Result<Option<CheckOutcome>, ShipError> {
        let request = &admission.request;
        if !request.options.reuse
            || !matches!(
                request.check_type,
                CheckType::Test | CheckType::Lint | CheckType::Typecheck
            )
            || !self
                .evidence
                .as_ref()
                .is_some_and(|port| port.can_reuse(request))
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
        // A newer failure or unfinished attempt must never be hidden by an
        // older pass for the same command and input identity.
        let Some(latest) = records
            .iter()
            .rev()
            .find(|receipt| receipt.id != admission.receipt_id)
        else {
            return Ok(None);
        };
        if !eligible(latest, admission) {
            return Ok(None);
        }
        let original = match latest.execution() {
            Some(CheckExecution::Reused { receipt_id }) => ledger.get(receipt_id)?,
            _ => Some(latest.clone()),
        };
        let Some(original) = original.filter(|receipt| {
            eligible(receipt, admission)
                && matches!(
                    receipt.execution(),
                    Some(CheckExecution::Exited { code: 0 })
                )
        }) else {
            return Ok(None);
        };
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
        if !admission.request.options.reuse
            || !self
                .evidence
                .as_ref()
                .is_some_and(|port| port.can_reuse(&admission.request))
            || !original.as_ref().is_some_and(|receipt| {
                eligible(receipt, admission)
                    && matches!(
                        receipt.execution(),
                        Some(CheckExecution::Exited { code: 0 })
                    )
            })
        {
            return Err(ShipError::refused(
                "invalid_reuse",
                "The earlier check no longer supplies matching verified evidence. Run the command again.",
            ));
        }
        Ok(())
    }
}

fn eligible(receipt: &CheckReceipt, admission: &CheckAdmission) -> bool {
    let request = &admission.request;
    if !receipt.is_applied()
        || !receipt.passed()
        || !receipt.verified()
        || receipt.gate != request.name
        || receipt.check_type != request.check_type
        || receipt.command != request.command
        || receipt.required != request.required
        || !matches!(receipt.freshness, Some(super::SourceFreshness::Unchanged))
    {
        return false;
    }
    let (Some(before), Some(now)) = (&receipt.source, &admission.source) else {
        return false;
    };
    if before.execution_digest.is_empty() || before.drift_to(now).is_some() {
        return false;
    }
    let Ok(recorded) = chrono::DateTime::parse_from_rfc3339(&receipt.admitted_at) else {
        return false;
    };
    let age = chrono::Utc::now().signed_duration_since(recorded);
    age.num_seconds() >= 0 && age < chrono::Duration::hours(24)
}
