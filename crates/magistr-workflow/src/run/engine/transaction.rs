use super::RunEngine;
use crate::infra::transaction::{TransactionError, TransactionTarget, transact};
use crate::run::application::RunError;
use crate::run::persistence::PersistedState;
use magistr_workflow_contracts::{Observed, ReadError, ReadErrorKind, ReadSource};

impl RunEngine {
    pub(super) fn persisted_state(&self) -> PersistedState {
        let mut state = PersistedState::empty(self.project_id.clone());
        state.goal = self.goal.clone();
        state.steps = self.steps.clone();
        state.execution = self.execution.clone();
        state.next_progress_id = self.next_progress_id;
        state.selection_revision = self.selection_revision;
        state.saved_cycles = self.saved_cycles.clone();
        state
    }

    pub(super) fn install(&mut self, state: PersistedState, source: ReadSource) {
        self.goal = state.goal;
        self.steps = state.steps;
        self.execution = state.execution;
        self.next_progress_id = state.next_progress_id;
        self.selection_revision = state.selection_revision;
        self.saved_cycles = state.saved_cycles;
        self.observation.record(Ok(Observed { value: (), source }));
    }

    pub(super) fn failed(&mut self, error: &RunError) {
        let read_error = match error {
            RunError::Store(error) => error.as_read_error(),
            RunError::Refused {
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
        step_ids: &[&str],
        apply: impl FnOnce(&mut PersistedState) -> Result<R, RunError>,
    ) -> Result<Observed<R>, RunError> {
        let source = self.observation.current()?.source;
        let state = self.persisted_state();
        let run = self.execution.run_id.clone();
        let steps = step_ids
            .iter()
            .map(|id| {
                self.execution
                    .steps
                    .get(*id)
                    .cloned()
                    .map(|instance| ((*id).to_owned(), instance))
                    .ok_or_else(|| RunError::from(format!("step '{id}' not found")))
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
                    || steps
                        .iter()
                        .any(|(id, instance)| candidate.execution.steps.get(id) != Some(instance))
                {
                    return Err(RunError::refused(
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
                    TransactionError::Store(error) => RunError::Store(error),
                };
                self.failed(&error);
                Err(error)
            }
        }
    }
}
