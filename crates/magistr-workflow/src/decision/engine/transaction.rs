//! Apply reasoning changes to the latest complete owner state, then publish it.
use std::collections::HashMap;

use magistr_workflow_contracts::{Observed, ReadSource};

use crate::decision::domain::{Log, LogEntry, NoteError};
use crate::decision::persistence::PersistedProject;
use crate::infra::transaction::{Committed, TransactionError};

use super::ReasoningServer;

impl ReasoningServer {
    pub(crate) fn selected_key(&self) -> String {
        self.active_log
            .clone()
            .unwrap_or_else(|| self.project_id.clone())
    }

    pub(crate) fn empty_selected_history(&self, selected: Option<&str>) -> Log {
        let mut history = Self::new_history();
        history.log_id = selected
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
            .logs
            .get(key)
            .cloned()
            .unwrap_or_else(|| self.empty_selected_history(selected.as_deref()));
        let old_access: HashMap<_, _> = self
            .logs
            .iter()
            .map(|(id, entry)| (id.clone(), entry.last_accessed))
            .collect();
        self.logs = if self.config.features.enable_logs {
            project
                .logs
                .iter()
                .map(|(id, history)| {
                    (
                        id.clone(),
                        LogEntry {
                            history: history.clone(),
                            last_accessed: old_access.get(id).copied().unwrap_or_else(Self::now_ms),
                        },
                    )
                })
                .collect()
        } else {
            HashMap::new()
        };
        self.project_notes = project
            .logs
            .iter()
            .flat_map(|(key, history)| {
                history
                    .notes
                    .iter()
                    .enumerate()
                    .map(move |(index, note)| (note.note, (key.clone(), index)))
            })
            .collect();
        self.project_max_note = self.project_notes.keys().copied().max().unwrap_or(0);
        self.append_state_validated = true;
        self.pinned_order = history
            .notes
            .iter()
            .filter(|note| note.pinned.unwrap_or(false))
            .map(|note| note.note)
            .collect();
        self.project = project;
        self.history = history;
        self.active_log = selected;
        self.threads = Self::restored_threads(&self.history);
        self.note_index.clear();
        self.note_numbers.clear();
        self.tools_used.clear();
        self.note_to_thread.clear();
        self.thread_depth_cache.clear();
        for (index, note) in self.history.notes.iter().enumerate() {
            self.note_index.insert(note.note, index);
            self.note_numbers.insert(note.note);
            if let Some(thread) = &note.thread_id {
                self.note_to_thread.insert(note.note, thread.clone());
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

    pub(crate) fn candidate(&self, project: &PersistedProject, selected: Option<String>) -> Self {
        let mut candidate = Self::for_analysis(Self::new_history(), HashMap::new());
        candidate.config = self.config.clone();
        candidate.project_id = self.project_id.clone();
        candidate.cwd = self.cwd.clone();
        candidate.start_time = self.start_time;
        candidate.notes_since_cleanup = self.notes_since_cleanup;
        candidate.install_project(project.clone(), selected, false);
        candidate
    }

    pub(crate) fn transact_history<R>(
        &mut self,
        selected: Option<String>,
        apply: impl FnOnce(&mut Self) -> Result<(R, Log), NoteError>,
    ) -> Result<Observed<R>, NoteError> {
        let result = self.persistence.transact(&self.project, |project| {
            let mut candidate = self.candidate(project, selected.clone());
            let (value, history) = apply(&mut candidate)?;
            let key = candidate.selected_key();
            project.logs.insert(key, history);
            Ok(value)
        });
        self.settle(result, selected)
    }

    pub(crate) fn transact_project<R>(
        &mut self,
        selected: Option<String>,
        apply: impl FnOnce(&mut PersistedProject) -> Result<R, NoteError>,
    ) -> Result<Observed<R>, NoteError> {
        let result = self.persistence.transact(&self.project, apply);
        self.settle(result, selected)
    }

    fn settle<R>(
        &mut self,
        result: Result<Committed<PersistedProject, R>, TransactionError<NoteError>>,
        selected: Option<String>,
    ) -> Result<Observed<R>, NoteError> {
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
                Err(NoteError::from_write(error))
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
