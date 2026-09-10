//! The loop's own operation ledger: what it wrote before an effect and
//! settled after, and how recovery establishes the outcome of an effect
//! whose owner was lost.

use magistr_domain::identity::{AttemptId, CommandId};
use magistr_domain::workflow::{
    OperationAdmission, OperationKind, OperationLedger, OperationOutcome, OperationRecord,
};
use magistr_engine::NativeEngines;

use super::{IterationLoop, git_head_sha};
use crate::error::{MagistrError, Result};
use crate::workflow::process_alive::is_process_alive;
use crate::workflow::reconciliation::Reconciliation;

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
    /// the runtime's own handle (`pid:1234`), kept apart from the command.
    /// Persist after it, so recovery knows what to ask.
    pub fn start_operation(&mut self, attempt: &AttemptId, external: Option<String>) -> Result<()> {
        self.state
            .operations
            .start(attempt, external)
            .map_err(operation_error)
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
    /// - a native mutation by the chunk's state in the native store;
    /// - a receipt by the checks recorded on the named task;
    /// - a publication is cancelled — attached surfaces reseed from the store
    ///   and there is nothing to redo;
    /// - a provider dispatch stays open while its runtime is still alive;
    ///   once it is gone, a commit past the attempt's baseline establishes
    ///   `Completed` and none establishes `Failed`.
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
                OperationKind::Receipt => self
                    .native
                    .as_ref()
                    .and_then(|engines| receipt_outcome(engines, &record.target)),
                OperationKind::ProviderDispatch => {
                    self.dispatch_outcome(record.external.as_deref())
                }
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

    /// A runtime still alive is nobody's to judge; a gone one is judged by
    /// the attempt's Git baseline. Without a handle there is no runtime to
    /// ask, and the baseline is the only evidence.
    fn dispatch_outcome(&self, external: Option<&str>) -> Option<OperationOutcome> {
        let pid = external
            .and_then(|handle| handle.strip_prefix("pid:"))
            .and_then(|pid| pid.parse::<u32>().ok());
        if pid.is_some_and(|pid| is_process_alive(pid) == Some(true)) {
            return None;
        }
        let head = git_head_sha(&self.project_dir);
        let committed = match (&self.state.last_commit_sha, &head) {
            (Some(before), Some(after)) => before != after,
            (None, Some(_)) => true,
            _ => false,
        };
        Some(if committed {
            OperationOutcome::Completed
        } else {
            OperationOutcome::Failed
        })
    }
}

/// `verb:chunk[:argument]` against the chunk as the store holds it now.
fn native_mutation_outcome(engines: &NativeEngines, target: &str) -> Option<OperationOutcome> {
    use magistr_engine::tas::ChunkStatus;
    engines.reload_roadmap();
    let (verb, rest) = target.split_once(':')?;
    let (id, argument) = match rest.split_once(':') {
        Some((id, argument)) => (id, Some(argument)),
        None => (rest, None),
    };
    let chunk = engines.roadmap_chunk(id);
    let applied = match verb {
        "start_chunk" => chunk.is_some_and(|chunk| {
            matches!(chunk.status, ChunkStatus::InProgress | ChunkStatus::Done)
        }),
        "complete_chunk" => chunk.is_some_and(|chunk| chunk.status == ChunkStatus::Done),
        "set_blocked_by" => chunk.is_some_and(|chunk| chunk.blocked_by.is_some()),
        "set_status" => chunk.is_some_and(|chunk| {
            serde_json::to_value(chunk.status)
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

/// `gate:<name>@task:<id>` against the checks the trace holds on that task.
fn receipt_outcome(engines: &NativeEngines, target: &str) -> Option<OperationOutcome> {
    let (gate, task) = target.strip_prefix("gate:")?.split_once("@task:")?;
    engines.reload_ship();
    let status = engines.ship_status();
    let recorded = status["checks"].as_array().is_some_and(|checks| {
        checks.iter().any(|check| {
            check["task_id"].as_str() == Some(task) && check["name"].as_str() == Some(gate)
        })
    });
    Some(if recorded {
        OperationOutcome::Completed
    } else {
        OperationOutcome::Failed
    })
}

fn operation_error(error: magistr_domain::workflow::OperationError) -> MagistrError {
    MagistrError::Workflow(error.to_string())
}

#[cfg(test)]
#[path = "iteration_operations.tests.rs"]
mod tests;
