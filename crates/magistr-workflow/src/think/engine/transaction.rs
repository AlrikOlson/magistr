//! Apply reasoning changes to the latest complete owner state, then publish it.
use std::collections::HashMap;

use magistr_workflow_contracts::{Observed, ReadSource};

use crate::infra::transaction::{Committed, TransactionError};
use crate::think::domain::{SessionEntry, StepError, ThinkHistory};
use crate::think::persistence::PersistedProject;

use super::ReasoningServer;

impl ReasoningServer {
    pub(crate) fn selected_key(&self) -> String {
        self.active_session
            .clone()
            .unwrap_or_else(|| self.project_id.clone())
    }

    pub(crate) fn empty_selected_history(&self, selected: Option<&str>) -> ThinkHistory {
        let mut history = Self::new_history();
        history.session_id = selected
            .filter(|id| *id != self.project_id)
            .map(str::to_owned);
        history
            .metadata
            .as_mut()
            .expect("new history metadata")
            .project_id = Some(self.project_id.clone());
        history
    }

    pub(crate) fn install_project(
        &mut self,
        project: PersistedProject,
        selected: Option<String>,
        trim: bool,
    ) {
        let key = selected.as_deref().unwrap_or(&self.project_id);
        let history = project
            .histories
            .get(key)
            .cloned()
            .unwrap_or_else(|| self.empty_selected_history(selected.as_deref()));
        let old_access: HashMap<_, _> = self
            .sessions
            .iter()
            .map(|(id, entry)| (id.clone(), entry.last_accessed))
            .collect();
        self.sessions = if self.config.features.enable_sessions {
            project
                .histories
                .iter()
                .map(|(id, history)| {
                    (
                        id.clone(),
                        SessionEntry {
                            history: history.clone(),
                            last_accessed: old_access.get(id).copied().unwrap_or_else(Self::now_ms),
                        },
                    )
                })
                .collect()
        } else {
            HashMap::new()
        };
        self.project = project;
        self.history = history;
        self.active_session = selected;
        self.branches = Self::restored_branches(&self.history);
        self.step_index.clear();
        self.step_numbers.clear();
        self.tools_used.clear();
        self.step_to_branch.clear();
        self.branch_depth_cache.clear();
        for (index, step) in self.history.steps.iter().enumerate() {
            self.step_index.insert(step.step_number, index);
            self.step_numbers.insert(step.step_number);
            if let Some(branch) = &step.branch_id {
                self.step_to_branch.insert(step.step_number, branch.clone());
            }
        }
        self.tools_used.extend(
            self.history
                .metadata
                .iter()
                .flat_map(|meta| meta.tools_used.iter().flatten().cloned()),
        );
        if trim {
            self.trim_history();
        }
    }

    fn candidate(&self, project: &PersistedProject, selected: Option<String>) -> Self {
        let mut candidate = Self::for_analysis(Self::new_history(), HashMap::new());
        candidate.config = self.config.clone();
        candidate.project_id = self.project_id.clone();
        candidate.cwd = self.cwd.clone();
        candidate.start_time = self.start_time;
        candidate.steps_since_cleanup = self.steps_since_cleanup;
        candidate.install_project(project.clone(), selected, false);
        candidate
    }

    pub(crate) fn transact_history<R>(
        &mut self,
        selected: Option<String>,
        apply: impl FnOnce(&mut Self) -> Result<(R, ThinkHistory), StepError>,
    ) -> Result<Observed<R>, StepError> {
        let result = self.persistence.transact(&self.project, |project| {
            let mut candidate = self.candidate(project, selected.clone());
            let (value, history) = apply(&mut candidate)?;
            let key = candidate.selected_key();
            project.histories.insert(key, history);
            Ok(value)
        });
        self.settle(result, selected)
    }

    pub(crate) fn transact_project<R>(
        &mut self,
        selected: Option<String>,
        apply: impl FnOnce(&mut PersistedProject) -> Result<R, StepError>,
    ) -> Result<Observed<R>, StepError> {
        let result = self.persistence.transact(&self.project, apply);
        self.settle(result, selected)
    }

    fn settle<R>(
        &mut self,
        result: Result<Committed<PersistedProject, R>, TransactionError<StepError>>,
        selected: Option<String>,
    ) -> Result<Observed<R>, StepError> {
        match result {
            Ok(committed) => {
                self.install_project(committed.state, selected, true);
                self.observation.record(Ok(Observed {
                    value: (),
                    source: committed.source,
                }));
                Ok(Observed {
                    value: committed.value,
                    source: committed.source,
                })
            }
            Err(TransactionError::Rejected(error)) => Err(error),
            Err(TransactionError::Store(error)) => {
                self.observation.record(Err(error.as_read_error()));
                Err(StepError::from_write(error))
            }
        }
    }

    /// The last mutation's observation, including a refused or uncertain write.
    pub fn mutation_state(&self) -> &magistr_workflow_contracts::ReadState<()> {
        &self.observation
    }

    pub(crate) fn memory_source() -> ReadSource {
        ReadSource::Memory
    }
}
