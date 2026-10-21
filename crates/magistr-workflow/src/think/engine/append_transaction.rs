//! Commit an append delta, then advance its derived views after success.

use super::{ReasoningServer, append::PreparedAppend};
use crate::infra::transaction::TransactionError;
use crate::think::domain::{RecordedStep, SessionEntry, StepError, ThinkStep};
use crate::think::persistence::PersistedProject;
use magistr_workflow_contracts::{Observed, ReadSource, WriteError};

impl ReasoningServer {
    pub(crate) fn transact_append(
        &mut self,
        selected: Option<String>,
        step: ThinkStep,
    ) -> Result<Observed<RecordedStep>, StepError> {
        let same_selection = self.active_session == selected;
        let (prepared, source, incremental) = if self.persistence.enabled() {
            let result = self.persistence.transact(&self.project, |project| {
                // Compare the actual validated contents read under the owner
                // lock, not just mtimes. Concurrent writers and in-place edits
                // cannot leave indexes pointing into a different history.
                let incremental = same_selection && project == &self.project;
                let prepared = if incremental {
                    self.prepare_append(step)?
                } else {
                    self.candidate(project, selected.clone())
                        .prepare_append(step)?
                };
                let key = selected.as_deref().unwrap_or(&self.project_id).to_owned();
                let history = project
                    .histories
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
                    return Err(StepError::from_write(error));
                }
            }
        } else {
            // Owner-created memory state is validated on construction. Imported
            // analysis state is validated once before its first mutation.
            if !self.append_state_validated {
                self.persistence
                    .validate(&self.project)
                    .map_err(|error| StepError::from_write(WriteError::read(error)))?;
                self.append_state_validated = true;
            }
            let prepared = if same_selection {
                self.prepare_append(step)?
            } else {
                self.candidate(&self.project, selected.clone())
                    .prepare_append(step)?
            };
            let key = selected.as_deref().unwrap_or(&self.project_id).to_owned();
            let empty = self.empty_selected_history(selected.as_deref());
            let history = self.project.histories.entry(key).or_insert(empty);
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
        let archive = self.project.histories.get(&key).expect("committed history");
        let number = prepared.step.step_number;
        self.project_steps
            .insert(number, (key.clone(), archive.steps.len() - 1));
        self.project_max_step = self.project_max_step.max(number);
        if self.config.features.enable_sessions {
            if let Some(entry) = self.sessions.get_mut(&key) {
                prepared.apply(&mut entry.history, prepared.revision_index);
                entry.last_accessed = Self::now_ms();
            } else {
                self.sessions.insert(
                    key,
                    SessionEntry {
                        history: archive.clone(),
                        last_accessed: Self::now_ms(),
                    },
                );
            }
        }
        let revision_index = prepared
            .step
            .revises_step
            .filter(|_| prepared.revision_index.is_some())
            .and_then(|number| self.step_index.get(&number))
            .copied();
        prepared.apply(&mut self.history, revision_index);
        if prepared.revision_index.is_some() {
            for member in self
                .branches
                .values_mut()
                .flat_map(|branch| &mut branch.steps)
            {
                if Some(member.step_number) == prepared.step.revises_step {
                    member.revised_by = Some(number);
                }
            }
        }
        if let Some(branch) = &prepared.new_branch {
            self.branches.insert(branch.id.clone(), branch.clone());
            self.invalidate_branch_depth_cache();
        }
        if let Some(id) = &prepared.step.branch_id {
            if let Some(branch) = self.branches.get_mut(id) {
                branch.steps.push(prepared.step.clone());
            }
            self.step_to_branch.insert(number, id.clone());
        }
        self.step_index.insert(number, self.history.steps.len() - 1);
        self.step_numbers.insert(number);
        self.tools_used.extend(prepared.tools_used.iter().cloned());
        if prepared.step.pinned.unwrap_or(false) {
            self.pinned_order.push(number);
        }
        self.trim_history();
    }
}
