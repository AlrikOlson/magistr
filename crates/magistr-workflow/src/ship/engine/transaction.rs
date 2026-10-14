use super::ShipEngine;
use crate::infra::transaction::{TransactionError, TransactionTarget, transact};
use crate::ship::application::ShipError;
use crate::ship::persistence::PersistedState;
use magistr_workflow_contracts::{Observed, ReadError, ReadErrorKind, ReadSource};

impl ShipEngine {
    pub(super) fn persisted_state(&self) -> PersistedState {
        let mut state = PersistedState::empty(self.project_id.clone());
        state.objective = self.objective.clone();
        state.tasks = self.tasks.clone();
        state.execution = self.execution.clone();
        state.next_action_id = self.next_action_id;
        state.selection_revision = self.selection_revision;
        state.saved_cycles = self.saved_cycles.clone();
        state
    }

    pub(super) fn install(&mut self, state: PersistedState, source: ReadSource) {
        self.objective = state.objective;
        self.tasks = state.tasks;
        self.execution = state.execution;
        self.next_action_id = state.next_action_id;
        self.selection_revision = state.selection_revision;
        self.saved_cycles = state.saved_cycles;
        self.observation.record(Ok(Observed { value: (), source }));
    }

    pub(super) fn failed(&mut self, error: &ShipError) {
        let read_error = match error {
            ShipError::Store(error) => error.as_read_error(),
            ShipError::Refused {
                kind: "stale_target",
                ..
            } => ReadError::new(ReadErrorKind::Changed, error.to_string()),
            _ => return,
        };
        self.observation.record(Err(read_error));
    }

    /// Capture intent from the observed owner; validate it against the latest
    /// locked record before changing any candidate. Only the committed candidate
    /// can replace live state.
    pub(super) fn mutate<R>(
        &mut self,
        task_ids: &[&str],
        apply: impl FnOnce(&mut PersistedState) -> Result<R, ShipError>,
    ) -> Result<Observed<R>, ShipError> {
        let source = self.observation.current()?.source;
        let state = self.persisted_state();
        let run = self.execution.run_id.clone();
        let tasks = task_ids
            .iter()
            .map(|id| {
                self.execution
                    .tasks
                    .get(*id)
                    .cloned()
                    .map(|instance| ((*id).to_owned(), instance))
                    .ok_or_else(|| ShipError::from(format!("task '{id}' not found")))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let empty = if source == ReadSource::Missing {
            // This empty identity was itself observed as missing. A different
            // writer's first commit will carry its own identity and be refused.
            state.clone()
        } else {
            PersistedState::empty(self.project_id.clone())
        };
        let path = self
            .persistence
            .as_ref()
            .map(|p| p.path_for(&self.project_id));
        let lock_path = self
            .persistence
            .as_ref()
            .map(|p| p.lock_path(&self.project_id));
        let target = match (&self.persistence, &path, &lock_path) {
            (Some(p), Some(path), Some(lock_path)) if p.enabled() => {
                TransactionTarget::File { path, lock_path }
            }
            _ => TransactionTarget::Memory(&state),
        };
        let result = transact(
            target,
            || empty,
            |candidate| candidate.validate(&self.project_id),
            |candidate| {
                if candidate.execution.run_id != run
                    || tasks
                        .iter()
                        .any(|(id, instance)| candidate.execution.tasks.get(id) != Some(instance))
                {
                    return Err(ShipError::refused(
                        "stale_target",
                        "The selected work changed. Read it again before making this change.",
                    ));
                }
                apply(candidate)
            },
        );
        match result {
            Ok(committed) => {
                self.install(committed.state, committed.source);
                Ok(Observed {
                    value: committed.value,
                    source: committed.source,
                })
            }
            Err(error) => {
                let error = match error {
                    TransactionError::Rejected(error) => error,
                    TransactionError::Store(error) => ShipError::Store(error),
                };
                self.failed(&error);
                Err(error)
            }
        }
    }
}
