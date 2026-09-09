use std::collections::{BTreeMap, BTreeSet};

use super::{AgreementError, AuthorityAction, InstructionLease, PlanRevision, WorkerPermit};

/// Admission history shares TaskAuthority's journal and cannot be mutated by the host.
#[derive(Debug, Default)]
pub(super) struct WorkerAdmission {
    records: BTreeMap<String, Record>,
    attempts: BTreeMap<String, u32>,
    completed: BTreeSet<String>,
    reserved: u32,
}

#[derive(Debug)]
struct Record {
    request: AuthorityAction,
    permit: WorkerPermit,
    receipt: Option<String>,
}

impl WorkerAdmission {
    pub(super) fn observe(&mut self, observed: u32) {
        self.reserved = self.reserved.max(observed);
    }

    pub(super) fn invalidate(&mut self, affected: &BTreeSet<String>) {
        self.completed.retain(|task| !affected.contains(task));
    }

    pub(super) fn invalidate_all(&mut self) {
        self.completed.clear();
    }

    pub(super) fn unsettled(&self) -> impl Iterator<Item = &WorkerPermit> {
        self.records
            .values()
            .filter(|record| record.receipt.is_none())
            .map(|record| &record.permit)
    }

    pub(super) fn receipt(&self, id: &str) -> Option<&str> {
        self.records.get(id)?.receipt.as_deref()
    }

    pub(super) fn active(&self, id: &str) -> Result<&WorkerPermit, AgreementError> {
        self.records
            .get(id)
            .filter(|record| record.receipt.is_none())
            .map(|record| &record.permit)
            .ok_or_else(|| AgreementError::Invalid("unknown or settled worker request".into()))
    }

    pub(super) fn validate_admission(
        &self,
        action: &AuthorityAction,
        revision: &PlanRevision,
        observed_attempts: u32,
    ) -> Result<(), AgreementError> {
        let AuthorityAction::AdmitWorker {
            id, task, worker, ..
        } = action
        else {
            return Err(AgreementError::Invalid("expected worker admission".into()));
        };
        super::require_text("worker request id", id)?;
        if let Some(record) = self.records.get(id) {
            return if &record.request == action {
                Ok(())
            } else {
                Err(AgreementError::Invalid(
                    "worker request id already used with different intent".into(),
                ))
            };
        }
        let body = revision.body();
        let planned = body
            .tasks
            .iter()
            .find(|planned| &planned.id == task)
            .ok_or_else(|| {
                AgreementError::Invalid("worker task is not in the approved plan".into())
            })?;
        if !body.worker_models.contains(worker) {
            return Err(AgreementError::Invalid(
                "worker model is not approved".into(),
            ));
        }
        if self.completed.contains(task)
            || self.unsettled().any(|permit| permit.lease.task() == task)
        {
            return Err(AgreementError::Invalid(
                "task is accepted or has an unsettled worker".into(),
            ));
        }
        if !planned
            .dependencies
            .iter()
            .all(|dependency| self.completed.contains(dependency))
        {
            return Err(AgreementError::Invalid(
                "worker dependencies are not accepted".into(),
            ));
        }
        let attempts = self.attempts.get(task).copied().unwrap_or(0);
        if attempts > body.retry_allowance || attempts == u32::MAX {
            return Err(AgreementError::Invalid(
                "worker retry allowance exhausted".into(),
            ));
        }
        if self.reserved.max(observed_attempts) >= revision.contract().limits.max_attempts {
            return Err(AgreementError::Invalid(
                "worker attempt reservations exhausted".into(),
            ));
        }
        Ok(())
    }

    /// Called only after validation and a successful observation.
    pub(super) fn admit(
        &mut self,
        action: &AuthorityAction,
        lease: InstructionLease,
    ) -> Option<WorkerPermit> {
        let AuthorityAction::AdmitWorker {
            id, task, worker, ..
        } = action
        else {
            return None;
        };
        if self.records.contains_key(id) {
            return None;
        }
        let attempt = self.attempts.entry(task.clone()).or_default();
        *attempt += 1; // checked before mutation
        self.reserved += 1; // checked against the u32 run ceiling
        let permit = WorkerPermit {
            id: id.clone(),
            lease,
            worker: worker.clone(),
            attempt: *attempt,
        };
        self.records.insert(
            id.clone(),
            Record {
                request: action.clone(),
                permit: permit.clone(),
                receipt: None,
            },
        );
        Some(permit)
    }

    /// A receipt is host-attributed evidence, not independently checked by this ledger.
    pub(super) fn settle(&mut self, id: &str, receipt: &str, accepted: bool) {
        if let Some(record) = self.records.get_mut(id) {
            record.receipt = Some(receipt.into());
            if accepted {
                self.completed.insert(record.permit.lease.task().into());
            }
        }
    }
}
