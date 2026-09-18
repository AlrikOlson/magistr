use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::num::NonZeroU32;
use serde::{Deserialize, Serialize};

use crate::identity::{AttemptId, CommandId};

use super::{
    DispatchPhase, OperationAdmission, OperationError, OperationKind, OperationOutcome,
    OperationRecord,
};

/// The run's own record of the operations it dispatched, one entry per
/// attempt. It is written before an effect and settled after it; it is not
/// a second authority on the effect's target — reconciliation asks the
/// native store, the runtime or Git what actually happened.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationLedger {
    records: Vec<OperationRecord>,
}

impl OperationLedger {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Admit `command` for `kind` on `target`.
    ///
    /// Deduplicates only where an outcome is established: a `Completed`
    /// latest attempt answers `Established`. A `Failed` or `Cancelled` one
    /// admits the next attempt number. An unsettled one — `Accepted`,
    /// `Running` or `Indeterminate` — refuses until it is settled or
    /// reconciled. Reusing a command id for another kind or target is a
    /// conflict, never a silent retry of something else.
    pub fn admit(
        &mut self,
        command: CommandId,
        kind: OperationKind,
        target: impl Into<String>,
    ) -> Result<OperationAdmission, OperationError> {
        let target = target.into();
        let latest = self
            .records
            .iter()
            .filter(|record| record.attempt.command() == &command)
            .max_by_key(|record| record.attempt.number());
        let number = match latest {
            None => NonZeroU32::MIN,
            Some(record) if record.kind != kind || record.target != target => {
                return Err(OperationError::Conflict(Box::new(record.clone())));
            }
            Some(record) => match record.outcome {
                OperationOutcome::Completed => {
                    return Ok(OperationAdmission::Established(record.clone()));
                }
                OperationOutcome::Failed | OperationOutcome::Cancelled => {
                    record.attempt.number().saturating_add(1)
                }
                OperationOutcome::Accepted
                | OperationOutcome::Running
                | OperationOutcome::Indeterminate => {
                    return Err(OperationError::Unsettled(Box::new(record.clone())));
                }
            },
        };
        let attempt = AttemptId::new(command, number);
        self.records.push(OperationRecord {
            attempt: attempt.clone(),
            kind,
            target,
            outcome: OperationOutcome::Accepted,
            external: None,
            dispatch_phase: (kind == OperationKind::ProviderDispatch)
                .then_some(DispatchPhase::Pending),
            resolution: None,
        });
        Ok(OperationAdmission::Accepted(attempt))
    }

    /// The effect was dispatched; `external` is the runtime's own handle.
    pub fn start(
        &mut self,
        attempt: &AttemptId,
        external: Option<String>,
    ) -> Result<(), OperationError> {
        let record = self.record_mut(attempt)?;
        // A dispatch checkpoints Running before spawn, then attaches its handle.
        // Never replace a runtime identity or revive a settled attempt.
        if record.kind == OperationKind::ProviderDispatch
            && record.outcome == OperationOutcome::Running
            && (record.external.is_none() || record.external == external)
        {
            if external.is_some() {
                record.external = external;
            }
            return Ok(());
        }
        Self::transition(
            record,
            OperationOutcome::Running,
            OperationOutcome::can_settle_to,
        )?;
        if external.is_some() {
            record.external = external;
        }
        Ok(())
    }

    /// The live owner reports `outcome`. Repeating a settled outcome is a
    /// no-op; any other change to a settled attempt is refused.
    pub fn settle(
        &mut self,
        attempt: &AttemptId,
        outcome: OperationOutcome,
    ) -> Result<(), OperationError> {
        let record = self.record_mut(attempt)?;
        if record.outcome == outcome && outcome.is_settled() {
            return Ok(());
        }
        Self::transition(record, outcome, OperationOutcome::can_settle_to)
    }

    /// Recovery establishes `outcome` for an `Indeterminate` attempt.
    pub fn reconcile(
        &mut self,
        attempt: &AttemptId,
        outcome: OperationOutcome,
    ) -> Result<(), OperationError> {
        let record = self.record_mut(attempt)?;
        Self::transition(record, outcome, OperationOutcome::can_reconcile_to)
    }

    /// Apply an explicit operator decision to one unsettled dispatch only.
    /// The adapter must first exclude live owners and confirm external settlement.
    pub fn resolve_dispatch(
        &mut self,
        attempt: &AttemptId,
        outcome: OperationOutcome,
        reason: &str,
    ) -> Result<(), OperationError> {
        let record = self.record_mut(attempt)?;
        if record.kind != OperationKind::ProviderDispatch || record.is_settled() {
            return Err(OperationError::Resolution(
                "Only an unfinished agent launch can be resolved.",
            ));
        }
        if !outcome.is_settled() || reason.trim().is_empty() {
            return Err(OperationError::Resolution(
                "Choose a final outcome and explain what happened.",
            ));
        }
        record.outcome = outcome;
        record.resolution = Some(reason.trim().into());
        Ok(())
    }

    /// The owner is gone: every attempt it had in flight is `Indeterminate`
    /// until reconciled. Returns the attempts so marked.
    pub fn mark_interrupted(&mut self) -> Vec<AttemptId> {
        self.records
            .iter_mut()
            .filter(|record| {
                matches!(
                    record.outcome,
                    OperationOutcome::Accepted | OperationOutcome::Running
                )
            })
            .map(|record| {
                record.outcome = OperationOutcome::Indeterminate;
                record.attempt.clone()
            })
            .collect()
    }

    /// Every attempt without an established outcome.
    pub fn unsettled(&self) -> impl Iterator<Item = &OperationRecord> {
        self.records.iter().filter(|record| !record.is_settled())
    }

    /// Every attempt reconciliation still has to establish.
    pub fn indeterminate(&self) -> impl Iterator<Item = &OperationRecord> {
        self.records
            .iter()
            .filter(|record| record.outcome == OperationOutcome::Indeterminate)
    }

    #[must_use]
    pub fn record(&self, attempt: &AttemptId) -> Option<&OperationRecord> {
        self.records
            .iter()
            .find(|record| &record.attempt == attempt)
    }

    #[must_use]
    pub fn records(&self) -> &[OperationRecord] {
        &self.records
    }

    /// Persisted evidence an adapter may install: attempts are unique and a
    /// command has at most one unsettled attempt.
    pub fn validate(&self) -> Result<(), &'static str> {
        for (index, record) in self.records.iter().enumerate() {
            if record.dispatch_phase.is_some() && record.kind != OperationKind::ProviderDispatch {
                return Err("launch phase belongs only to provider dispatches");
            }
            if record.dispatch_phase == Some(DispatchPhase::Pending)
                && (record.external.is_some() || record.outcome == OperationOutcome::Running)
            {
                return Err("a pending dispatch cannot have a runtime or be running");
            }
            let same_command = self.records[..index]
                .iter()
                .filter(|earlier| earlier.attempt.command() == record.attempt.command());
            for earlier in same_command {
                if earlier.attempt == record.attempt {
                    return Err("operation attempt recorded twice");
                }
                if earlier.kind != record.kind || earlier.target != record.target {
                    return Err("one command id names two different operations");
                }
                if !earlier.is_settled() && !record.is_settled() {
                    return Err("a command has two unsettled attempts");
                }
            }
        }
        Ok(())
    }

    fn record_mut(&mut self, attempt: &AttemptId) -> Result<&mut OperationRecord, OperationError> {
        self.records
            .iter_mut()
            .find(|record| &record.attempt == attempt)
            .ok_or_else(|| OperationError::Unknown(attempt.clone()))
    }

    fn transition(
        record: &mut OperationRecord,
        to: OperationOutcome,
        allowed: fn(OperationOutcome, OperationOutcome) -> bool,
    ) -> Result<(), OperationError> {
        if !allowed(record.outcome, to) {
            return Err(OperationError::Transition {
                attempt: record.attempt.clone(),
                from: record.outcome,
                to,
            });
        }
        record.outcome = to;
        if to == OperationOutcome::Running && record.kind == OperationKind::ProviderDispatch {
            record.dispatch_phase = Some(DispatchPhase::Started);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "operation_ledger.tests.rs"]
mod tests;
