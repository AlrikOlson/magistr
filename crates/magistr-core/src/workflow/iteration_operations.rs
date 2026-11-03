//! The loop's own operation ledger: what it wrote before an effect and
//! settled after, and how recovery establishes the outcome of an effect
//! whose owner was lost.

use magistr_domain::identity::{AttemptId, CommandId};
use magistr_domain::workflow::{
    DispatchPhase, OperationAdmission, OperationKind, OperationLedger, OperationOutcome,
    OperationRecord,
};
use magistr_engine::NativeReader;

use super::{IterationLoop, git_head_advanced_from, git_head_sha};
use crate::error::{MagistrError, Result};
use crate::workflow::reconciliation::Reconciliation;
use crate::workflow::runtime_handle::RuntimeHandle;

impl IterationLoop {
    /// Admit `key` for this attempt: the record is `Accepted`. The command is
    /// scoped to the run and the attempt cursor, so the same key next attempt
    /// is a new command and deduplication is per attempt.
    ///
    /// Returns `None` when the command's outcome is already established
    /// (`Completed`): nothing is redone. The record is in memory only —
    /// persist before dispatching the effect it names.
    pub fn begin_operation(
        &mut self,
        kind: OperationKind,
        key: &str,
        target: &str,
    ) -> Result<Option<AttemptId>> {
        let command = CommandId::new(
            self.state.run_id.clone(),
            format!("{}/{key}", self.state.run.completed_attempts),
        )
        .map_err(|error| MagistrError::Workflow(format!("operation key {key:?}: {error}")))?;
        match self
            .state
            .operations
            .admit(command, kind, target)
            .map_err(operation_error)?
        {
            OperationAdmission::Accepted(attempt) => Ok(Some(attempt)),
            OperationAdmission::Established(_) => Ok(None),
        }
    }

    /// The effect was dispatched: `attempt` is `Running`, and `external` is
    /// the effect's own identity, kept apart from the command — a
    /// [`RuntimeHandle`] for a provider dispatch, `check_result:<id>` for a
    /// check_result. Persist after it, so recovery knows what to ask.
    pub fn start_operation(&mut self, attempt: &AttemptId, external: Option<String>) -> Result<()> {
        self.state
            .operations
            .start(attempt, external)
            .map_err(operation_error)
    }

    /// The transport must call this before spawn. Failure refuses the launch.
    pub fn checkpoint_dispatch_launch(&mut self, attempt: &AttemptId) -> Result<()> {
        if self
            .state
            .operations
            .record(attempt)
            .is_none_or(|record| record.kind != OperationKind::ProviderDispatch)
        {
            return Err(MagistrError::Workflow(
                "This operation is not an agent launch.".into(),
            ));
        }
        self.start_operation(attempt, None)?;
        self.persist()
    }

    /// The live owner's report on `attempt`. Persist after it.
    pub fn settle_operation(
        &mut self,
        attempt: &AttemptId,
        outcome: OperationOutcome,
    ) -> Result<()> {
        self.state
            .operations
            .settle(attempt, outcome)
            .map_err(operation_error)
    }

    /// The run's operation records.
    #[must_use]
    pub fn operations(&self) -> &OperationLedger {
        &self.state.operations
    }

    /// What recovery established when this loop was opened; empty for a
    /// fresh run and for a recovery that found nothing in flight.
    #[must_use]
    pub fn reconciliation(&self) -> &Reconciliation {
        &self.reconciliation
    }

    /// Establish what happened to every attempt the lost owner left in
    /// flight, by asking the authority on each effect rather than the record:
    ///
    /// - a reservation is established by the checkpoint that carries it;
    /// - a native mutation by the task's state in the native store;
    /// - a check_result by its own durable check_result, read by the id the record
    ///   carries: applied → `Completed`; unsettled, stale or never
    ///   published → `Failed`. No check of the same name stands in for it;
    /// - a publication is cancelled — attached surfaces reseed from the store
    ///   and there is nothing to redo;
    /// - a provider dispatch still Pending never launched and fails. A
    ///   Started or legacy dispatch stays open without a handle, or while
    ///   that runtime lives. Once the runtime is gone, HEAD advancing from
    ///   the attempt's baseline establishes Completed, otherwise Failed.
    ///
    /// Anything the loop cannot ask about stays open, and an open attempt
    /// refuses the next admission.
    pub(super) fn reconcile(&mut self) -> Reconciliation {
        let mut reconciliation = Reconciliation::default();
        let open: Vec<OperationRecord> = self.state.operations.indeterminate().cloned().collect();
        for record in open {
            let established = match record.kind {
                OperationKind::Reservation => Some(OperationOutcome::Completed),
                OperationKind::Publication => Some(OperationOutcome::Cancelled),
                OperationKind::NativeMutation => self
                    .native
                    .as_ref()
                    .and_then(|engines| native_mutation_outcome(engines, &record.target)),
                OperationKind::CheckResult => self
                    .native
                    .as_ref()
                    .and_then(|engines| check_result_outcome(engines, &record)),
                OperationKind::ProviderDispatch => self.dispatch_outcome(&record),
            };
            match established {
                Some(outcome)
                    if self
                        .state
                        .operations
                        .reconcile(&record.attempt, outcome)
                        .is_ok() =>
                {
                    reconciliation
                        .settled
                        .push(OperationRecord { outcome, ..record });
                }
                _ => reconciliation.open.push(record),
            }
        }
        reconciliation
    }

    /// Pending proves the transport never crossed its pre-spawn checkpoint.
    /// Otherwise only the particular runtime's handle can establish its exit;
    /// a handle-less or legacy dispatch remains open even if HEAD advanced.
    fn dispatch_outcome(&self, record: &OperationRecord) -> Option<OperationOutcome> {
        if record.dispatch_phase == Some(DispatchPhase::Pending) && record.external.is_none() {
            return Some(OperationOutcome::Failed);
        }
        let handle = RuntimeHandle::parse(record.external.as_deref()?)?;
        if handle.is_live()? {
            return None;
        }
        let advanced = match &self.state.last_commit_sha {
            Some(baseline) => git_head_advanced_from(&self.project_dir, baseline)?,
            // No baseline to advance from: the first commit of a repository
            // is an advance, and no commit at all is none.
            None => git_head_sha(&self.project_dir).is_some(),
        };
        Some(if advanced {
            OperationOutcome::Completed
        } else {
            OperationOutcome::Failed
        })
    }
}

/// `verb:task[:argument]` against the task as the store holds it now.
fn native_mutation_outcome(engines: &NativeReader, target: &str) -> Option<OperationOutcome> {
    use magistr_engine::native::TaskStatus;
    let (verb, rest) = target.split_once(':')?;
    let (id, argument) = match rest.split_once(':') {
        Some((id, argument)) => (id, Some(argument)),
        None => (rest, None),
    };
    let task = match engines.plan_task(id) {
        Ok(task) => task,
        Err(error) => {
            tracing::warn!(%error, "native mutation outcome could not be observed");
            return None;
        }
    };
    let applied = match verb {
        "start_task" => task
            .is_some_and(|task| matches!(task.status, TaskStatus::InProgress | TaskStatus::Done)),
        "complete_task" => task.is_some_and(|task| task.status == TaskStatus::Done),
        "set_waiting_on" => task.is_some_and(|task| task.waiting_on.is_some()),
        "set_status" => task.is_some_and(|task| {
            serde_json::to_value(task.status)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
                .as_deref()
                == argument
        }),
        _ => return None,
    };
    Some(if applied {
        OperationOutcome::Completed
    } else {
        OperationOutcome::Failed
    })
}

/// `check:<name>@step:<id>` against the durable check_result the record names
/// (`check_result:<id>`), never against a same-named check on the step.
///
/// A result is applied only at settlement, and the loop settles only after
/// it has persisted the record as running under the check_result's id; so a
/// record with no id, or whose check_result the ledger never saw or never
/// settled, had nothing applied: `Failed`, and the next set admits a new
/// attempt. A settlement kept stale was not applied either. A check_result that
/// names another check or step than the record does is not this record's,
/// and the record stays open.
fn check_result_outcome(
    engines: &NativeReader,
    record: &OperationRecord,
) -> Option<OperationOutcome> {
    use magistr_engine::native::CheckResultSettlement;
    let (check, step) = record.target.strip_prefix("check:")?.split_once("@step:")?;
    let Some(id) = record
        .external
        .as_deref()
        .and_then(|handle| handle.strip_prefix("check_result:"))
    else {
        return Some(OperationOutcome::Failed);
    };
    let Some(check_result) = engines.run_check_result(id).ok()? else {
        return Some(OperationOutcome::Failed);
    };
    if check_result.check != check || check_result.target.step_id != step {
        return None;
    }
    Some(match check_result.settlement {
        Some(CheckResultSettlement::Applied) => OperationOutcome::Completed,
        Some(CheckResultSettlement::Stale { .. }) | None => OperationOutcome::Failed,
    })
}

fn operation_error(error: magistr_domain::workflow::OperationError) -> MagistrError {
    MagistrError::Workflow(error.to_string())
}

#[cfg(test)]
#[path = "iteration_operations.tests.rs"]
mod tests;
