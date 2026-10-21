//! Validate an append before touching state, then apply only its changed records.

use super::ReasoningServer;
use crate::think::config::namespace_session_id;
use crate::think::constants::{COMPLETION_PHRASES, VALID_PREFIXES, VALID_PURPOSES};
use crate::think::domain::{
    Branch, BranchStatus, HistoryMetadata, StepError, ThinkHistory, ThinkStep,
};
use chrono::Utc;
use std::collections::HashSet;
use std::time::Instant;

pub(crate) struct PreparedAppend {
    pub step: ThinkStep,
    pub auto_numbered: Option<u32>,
    pub recovered_fields: Vec<&'static str>,
    pub tools_used: Vec<String>,
    pub new_branch: Option<Branch>,
    pub revision_index: Option<usize>,
    pub metadata: Option<HistoryMetadata>,
    pub completed: bool,
    pub updated_at: String,
    pub created_at: Option<String>,
}

impl PreparedAppend {
    /// Validation is complete. Applying this delta has no fallible operations.
    pub fn apply(&self, history: &mut ThinkHistory, revision_index: Option<usize>) {
        if let Some(index) = revision_index {
            history.steps[index].revised_by = Some(self.step.step_number);
        }
        if let Some(branches) = history.branches.as_mut() {
            if self.revision_index.is_some() {
                for member in branches.iter_mut().flat_map(|branch| &mut branch.steps) {
                    if Some(member.step_number) == self.step.revises_step {
                        member.revised_by = Some(self.step.step_number);
                    }
                }
            }
        }
        if let Some(branch) = &self.new_branch {
            history
                .branches
                .get_or_insert_with(Vec::new)
                .push(branch.clone());
        }
        if let Some(id) = &self.step.branch_id {
            if let Some(branch) = history
                .branches
                .iter_mut()
                .flatten()
                .find(|branch| &branch.id == id)
            {
                branch.steps.push(self.step.clone());
            }
        }
        history.completed = self.completed;
        history.metadata.clone_from(&self.metadata);
        history.updated_at = Some(self.updated_at.clone());
        history.created_at.clone_from(&self.created_at);
        history.steps.push(self.step.clone());
    }
}

impl ReasoningServer {
    fn resolve_append_session(&self, step: &mut ThinkStep) {
        // Resolve the session id for this step. Any caller-supplied id
        // gets auto-prefixed with the project namespace so agents can't
        // accidentally write outside their project's bucket — see
        // config::namespace_session_id. When sessions aren't enabled
        // we leave session_id as supplied and write the default archive.
        if self.config.features.enable_sessions && !self.project_id.is_empty() {
            let raw = step
                .session_id
                .clone()
                .filter(|s| !s.trim().is_empty())
                .or_else(|| self.config.features.default_session_id.clone())
                .unwrap_or_else(|| self.project_id.clone());
            let resolved = namespace_session_id(&self.project_id, &raw);
            step.session_id = Some(resolved);
        }
    }

    pub(crate) fn prepare_append(&self, mut step: ThinkStep) -> Result<PreparedAppend, StepError> {
        let step_start = Instant::now();

        // Try to recover from XML-injection corruption BEFORE validating.
        // If the agent's `thought` contains embedded `<parameter name=X>VALUE</parameter>`
        // segments, we extract them and fill in the corresponding empty
        // fields. The caller (the model) sees a successful step plus a
        // warning that we recovered — far better than a hard reject.
        let recovered_fields = Self::recover_xml_injection(&mut step);

        // Every call appends a new record, including branches and revisions.
        // Allocate before required-field validation so omitted numbers work
        // consistently; references to earlier records keep their own fields.
        let mut auto_numbered: Option<u32> = None;
        if step.step_number == 0 || self.step_number_taken_in_project(step.step_number) {
            let requested = step.step_number;
            step.step_number = self
                .max_step_number_in_project()
                .checked_add(1)
                .ok_or_else(|| self.make_error("There are no unused reasoning numbers left."))?;
            auto_numbered = Some(requested);
        }

        if let Err(e) = Self::validate_required_fields(&step, &recovered_fields) {
            return Err(self.make_error(&e));
        }
        if let Err(e) = Self::validate_confidence(&step) {
            return Err(self.make_error(&e));
        }

        if self.config.validation.strict_mode {
            if !self.validate_thought_prefix(&step.thought) {
                return Err(self.make_error(&format!(
                    "Thought must start with one of: {} (strict mode)",
                    VALID_PREFIXES.join(", ")
                )));
            }
            if !self.validate_rationale(&step.rationale) {
                return Err(self.make_error("Rationale must start with \"To \" (strict mode)"));
            }
            if !self.validate_purpose(&step.purpose) {
                return Err(self.make_error(&format!(
                    "Invalid purpose \"{}\". Valid: {} (strict mode)",
                    step.purpose,
                    VALID_PURPOSES.join(", ")
                )));
            }
        } else if !self.validate_purpose(&step.purpose) {
            tracing::debug!("⚠️ Using custom purpose: {}", step.purpose);
        }

        step.timestamp = Some(Utc::now().to_rfc3339());
        // Stamp project root so the step carries its provenance to
        // disk. Cached at server-construction time — one syscall per
        // server, not per step.
        if step.cwd.is_none() {
            step.cwd = self.cwd.clone();
        }

        self.resolve_append_session(&mut step);

        if let Err(e) = self.validate_session_step(&mut step) {
            return Err(self.make_error(&e));
        }
        let completed = self.history.completed || step.is_final_step.unwrap_or(false) || {
            let lower = step.thought.to_ascii_lowercase();
            COMPLETION_PHRASES.iter().any(|p| lower.contains(p))
        };

        let revision_index = step
            .revises_step
            .filter(|_| self.config.features.enable_revisions)
            .and_then(|number| self.project_steps.get(&number))
            .filter(|(key, _)| key == &self.selected_key())
            .map(|(_, index)| *index);
        let new_branch = if let Some(from) = step.branch_from {
            let id = step
                .branch_id
                .clone()
                .unwrap_or_else(|| format!("branch-{}", uuid::Uuid::new_v4()));
            let name = step
                .branch_name
                .clone()
                .unwrap_or_else(|| format!("Alternative {}", self.branches.len() + 1));
            let new = (!self.branches.contains_key(&id)).then(|| Branch {
                id: id.clone(),
                name: name.clone(),
                from_step: from,
                steps: Vec::new(),
                status: BranchStatus::Active,
                created_at: Utc::now().to_rfc3339(),
                depth: self
                    .archived_step(from)
                    .and_then(|step| step.branch_id.as_ref())
                    .and_then(|id| self.branches.get(id))
                    .map_or(1, |branch| branch.depth + 1),
                merged_into: None,
            });
            step.branch_id = Some(id);
            step.branch_name = Some(name);
            new
        } else {
            None
        };
        let tools_used = Self::extract_tools_used(&step);
        let mut metadata = self.history.metadata.clone();
        if let Some(meta) = metadata.as_mut() {
            let mut tools: HashSet<_> = self.tools_used.iter().cloned().collect();
            tools.extend(tools_used.iter().cloned());
            let mut sorted: Vec<_> = tools.into_iter().collect();
            sorted.sort();
            meta.tools_used = Some(sorted);
            meta.project_id = Some(self.project_id.clone());
            meta.total_duration_ms =
                Some(u64::try_from(self.start_time.elapsed().as_millis()).unwrap_or(u64::MAX));
            if step.revises_step.is_some() && self.config.features.enable_revisions {
                meta.revisions_count = Some(meta.revisions_count.unwrap_or(0) + 1);
            }
            if new_branch.is_some() {
                meta.branches_created = Some(meta.branches_created.unwrap_or(0) + 1);
            }
        }
        step.duration_ms =
            Some(u64::try_from(step_start.elapsed().as_millis()).unwrap_or(u64::MAX));
        Ok(PreparedAppend {
            step,
            auto_numbered,
            recovered_fields,
            tools_used,
            new_branch,
            revision_index,
            metadata,
            completed,
            updated_at: Utc::now().to_rfc3339(),
            created_at: self.history.created_at.clone(),
        })
    }
}
