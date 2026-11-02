//! Commit an append delta, then advance its derived views after success.

use super::{ReasoningServer, append::PreparedAppend};
use crate::decision::domain::{LogEntry, Note, NoteError, RecordedNote};
use crate::decision::persistence::PersistedProject;
use crate::infra::transaction::TransactionError;
use magistr_workflow_contracts::{Observed, ReadSource, WriteError};

impl ReasoningServer {
    pub(crate) fn transact_append(
        &mut self,
        selected: Option<String>,
        note: Note,
    ) -> Result<Observed<RecordedNote>, NoteError> {
        let same_selection = self.active_log == selected;
        let (prepared, source, incremental) = if self.persistence.enabled() {
            let result = self.persistence.transact(&self.project, |project| {
                // Compare the actual validated contents read under the owner
                // lock, not just mtimes. Concurrent writers and in-place edits
                // cannot leave indexes pointing into a different history.
                let incremental = same_selection && project == &self.project;
                let prepared = if incremental {
                    self.prepare_append(note)?
                } else {
                    self.candidate(project, selected.clone())
                        .prepare_append(note)?
                };
                let key = selected.as_deref().unwrap_or(&self.project_id).to_owned();
                let history = project
                    .logs
                    .entry(key)
                    .or_insert_with(|| self.empty_selected_history(selected.as_deref()));
                prepared.apply(history, prepared.revision_index);
                Ok((prepared, incremental))
            });
            match result {
                Ok(committed) => {
                    let (prepared, incremental) = committed.value;
                    self.project = committed.state;
                    (prepared, committed.source, incremental)
                }
                Err(TransactionError::Rejected(error)) => return Err(error),
                Err(TransactionError::Store(error)) => {
                    self.observation.record(Err(error.as_read_error()));
                    return Err(NoteError::from_write(error));
                }
            }
        } else {
            // Owner-created memory state is validated on construction. Imported
            // analysis state is validated once before its first mutation.
            if !self.append_state_validated {
                self.persistence
                    .validate(&self.project)
                    .map_err(|error| NoteError::from_write(WriteError::read(error)))?;
                self.append_state_validated = true;
            }
            let prepared = if same_selection {
                self.prepare_append(note)?
            } else {
                self.candidate(&self.project, selected.clone())
                    .prepare_append(note)?
            };
            let key = selected.as_deref().unwrap_or(&self.project_id).to_owned();
            let empty = self.empty_selected_history(selected.as_deref());
            let history = self.project.logs.entry(key).or_insert(empty);
            // No clone/rollback is needed: preparation is read-only and the
            // accepted delta cannot fail while applying to in-memory state.
            prepared.apply(history, prepared.revision_index);
            (prepared, ReadSource::Memory, same_selection)
        };
        if incremental {
            self.advance_append_views(&prepared);
        } else {
            let project =
                std::mem::replace(&mut self.project, PersistedProject::empty(&self.project_id));
            self.install_project(project, selected, true);
        }
        self.observation.record(Ok(Observed { value: (), source }));
        Ok(Observed {
            value: self.append_report(&prepared),
            source,
        })
    }

    fn advance_append_views(&mut self, prepared: &PreparedAppend) {
        let key = self.selected_key();
        let archive = self.project.logs.get(&key).expect("committed history");
        let number = prepared.note.note;
        self.project_notes
            .insert(number, (key.clone(), archive.notes.len() - 1));
        self.project_max_note = self.project_max_note.max(number);
        if self.config.features.enable_logs {
            if let Some(entry) = self.logs.get_mut(&key) {
                prepared.apply(&mut entry.history, prepared.revision_index);
                entry.last_accessed = Self::now_ms();
            } else {
                self.logs.insert(
                    key,
                    LogEntry {
                        history: archive.clone(),
                        last_accessed: Self::now_ms(),
                    },
                );
            }
        }
        let revision_index = prepared
            .note
            .revises_note
            .filter(|_| prepared.revision_index.is_some())
            .and_then(|number| self.note_index.get(&number))
            .copied();
        prepared.apply(&mut self.history, revision_index);
        if prepared.revision_index.is_some() {
            for member in self
                .threads
                .values_mut()
                .flat_map(|thread| &mut thread.notes)
            {
                if Some(member.note) == prepared.note.revises_note {
                    member.revised_by = Some(number);
                }
            }
        }
        if let Some(thread) = &prepared.new_thread {
            self.threads.insert(thread.id.clone(), thread.clone());
            self.invalidate_thread_depth_cache();
        }
        if let Some(id) = &prepared.note.thread_id {
            if let Some(thread) = self.threads.get_mut(id) {
                thread.notes.push(prepared.note.clone());
            }
            self.note_to_thread.insert(number, id.clone());
        }
        self.note_index.insert(number, self.history.notes.len() - 1);
        self.note_numbers.insert(number);
        self.tools_used.extend(prepared.tools_used.iter().cloned());
        if prepared.note.pinned.unwrap_or(false) {
            self.pinned_order.push(number);
        }
        self.trim_history();
    }
}
