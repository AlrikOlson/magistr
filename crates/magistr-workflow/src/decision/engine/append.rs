//! Validate an append before touching state, then apply only its changed records.

use super::ReasoningServer;
use crate::decision::config::namespace_log_id;
use crate::decision::constants::{COMPLETION_PHRASES, VALID_PREFIXES, VALID_PURPOSES};
use crate::decision::domain::{Log, LogMetadata, Note, NoteError, Thread, ThreadStatus};
use chrono::Utc;
use std::collections::HashSet;
use std::time::Instant;

pub(crate) struct PreparedAppend {
    pub note: Note,
    pub auto_numbered: Option<u32>,
    pub recovered_fields: Vec<&'static str>,
    pub tools_used: Vec<String>,
    pub new_thread: Option<Thread>,
    pub revision_index: Option<usize>,
    pub metadata: Option<LogMetadata>,
    pub completed: bool,
    pub updated_at: String,
    pub created_at: Option<String>,
}

impl PreparedAppend {
    /// Validation is complete. Applying this delta has no fallible operations.
    pub fn apply(&self, history: &mut Log, revision_index: Option<usize>) {
        if let Some(index) = revision_index {
            history.notes[index].revised_by = Some(self.note.note);
        }
        if let Some(threads) = history.threads.as_mut() {
            if self.revision_index.is_some() {
                for member in threads.iter_mut().flat_map(|thread| &mut thread.notes) {
                    if Some(member.note) == self.note.revises_note {
                        member.revised_by = Some(self.note.note);
                    }
                }
            }
        }
        if let Some(thread) = &self.new_thread {
            history
                .threads
                .get_or_insert_with(Vec::new)
                .push(thread.clone());
        }
        if let Some(id) = &self.note.thread_id {
            if let Some(thread) = history
                .threads
                .iter_mut()
                .flatten()
                .find(|thread| &thread.id == id)
            {
                thread.notes.push(self.note.clone());
            }
        }
        history.completed = self.completed;
        history.metadata.clone_from(&self.metadata);
        history.updated_at = Some(self.updated_at.clone());
        history.created_at.clone_from(&self.created_at);
        history.notes.push(self.note.clone());
    }
}

impl ReasoningServer {
    fn resolve_append_log(&self, note: &mut Note) {
        // Resolve the log id for this note. Any caller-supplied id
        // gets auto-prefixed with the project namespace so agents can't
        // accidentally write outside their project's bucket — see
        // config::namespace_log_id. When logs aren't enabled
        // we leave log_id as supplied and write the default archive.
        if self.config.features.enable_logs && !self.project_id.is_empty() {
            let raw = note
                .log_id
                .clone()
                .filter(|s| !s.trim().is_empty())
                .or_else(|| self.config.features.default_log_id.clone())
                .unwrap_or_else(|| self.project_id.clone());
            let resolved = namespace_log_id(&self.project_id, &raw);
            note.log_id = Some(resolved);
        }
    }

    pub(crate) fn prepare_append(&self, mut note: Note) -> Result<PreparedAppend, NoteError> {
        let note_start = Instant::now();

        // Try to recover from XML-injection corruption BEFORE validating.
        // If the agent's `thought` contains embedded `<parameter name=X>VALUE</parameter>`
        // segments, we extract them and fill in the corresponding empty
        // fields. The caller (the model) sees a successful note plus a
        // warning that we recovered — far better than a hard reject.
        let recovered_fields = Self::recover_xml_injection(&mut note);

        // Every call appends a new record, including threads and revisions.
        // Allocate before required-field validation so omitted numbers work
        // consistently; references to earlier records keep their own fields.
        let mut auto_numbered: Option<u32> = None;
        if note.note == 0 || self.note_number_taken_in_project(note.note) {
            let requested = note.note;
            note.note = self
                .max_note_number_in_project()
                .checked_add(1)
                .ok_or_else(|| self.make_error("There are no unused reasoning numbers left."))?;
            auto_numbered = Some(requested);
        }

        if let Err(e) = Self::validate_required_fields(&note, &recovered_fields) {
            return Err(self.make_error(&e));
        }
        if let Err(e) = Self::validate_confidence(&note) {
            return Err(self.make_error(&e));
        }

        if self.config.validation.strict_mode {
            if !self.validate_thought_prefix(&note.thought) {
                return Err(self.make_error(&format!(
                    "Thought must start with one of: {} (strict mode)",
                    VALID_PREFIXES.join(", ")
                )));
            }
            if !self.validate_rationale(&note.rationale) {
                return Err(self.make_error("Rationale must start with \"To \" (strict mode)"));
            }
            if !self.validate_purpose(&note.purpose) {
                return Err(self.make_error(&format!(
                    "Invalid purpose \"{}\". Valid: {} (strict mode)",
                    note.purpose,
                    VALID_PURPOSES.join(", ")
                )));
            }
        } else if !self.validate_purpose(&note.purpose) {
            tracing::debug!("⚠️ Using custom purpose: {}", note.purpose);
        }

        note.timestamp = Some(Utc::now().to_rfc3339());
        // Stamp project root so the note carries its provenance to
        // disk. Cached at server-construction time — one syscall per
        // server, not per note.
        if note.cwd.is_none() {
            note.cwd = self.cwd.clone();
        }

        self.resolve_append_log(&mut note);

        if let Err(e) = self.validate_log_note(&mut note) {
            return Err(self.make_error(&e));
        }
        let completed = self.history.completed || note.is_final_note.unwrap_or(false) || {
            let lower = note.thought.to_ascii_lowercase();
            COMPLETION_PHRASES.iter().any(|p| lower.contains(p))
        };

        let revision_index = note
            .revises_note
            .filter(|_| self.config.features.enable_revisions)
            .and_then(|number| self.project_notes.get(&number))
            .filter(|(key, _)| key == &self.selected_key())
            .map(|(_, index)| *index);
        let new_thread = if let Some(from) = note.thread_from {
            let id = note
                .thread_id
                .clone()
                .unwrap_or_else(|| format!("thread-{}", uuid::Uuid::new_v4()));
            let name = note
                .thread_name
                .clone()
                .unwrap_or_else(|| format!("Alternative {}", self.threads.len() + 1));
            let new = (!self.threads.contains_key(&id)).then(|| Thread {
                id: id.clone(),
                name: name.clone(),
                from_note: from,
                notes: Vec::new(),
                status: ThreadStatus::Open,
                created_at: Utc::now().to_rfc3339(),
                depth: self
                    .archived_note(from)
                    .and_then(|note| note.thread_id.as_ref())
                    .and_then(|id| self.threads.get(id))
                    .map_or(1, |thread| thread.depth + 1),
                merged_into: None,
            });
            note.thread_id = Some(id);
            note.thread_name = Some(name);
            new
        } else {
            None
        };
        let tools_used = Self::extract_tools_used(&note);
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
            if note.revises_note.is_some() && self.config.features.enable_revisions {
                meta.revisions_count = Some(meta.revisions_count.unwrap_or(0) + 1);
            }
            if new_thread.is_some() {
                meta.threads_created = Some(meta.threads_created.unwrap_or(0) + 1);
            }
        }
        note.duration_ms =
            Some(u64::try_from(note_start.elapsed().as_millis()).unwrap_or(u64::MAX));
        Ok(PreparedAppend {
            note,
            auto_numbered,
            recovered_fields,
            tools_used,
            new_thread,
            revision_index,
            metadata,
            completed,
            updated_at: Utc::now().to_rfc3339(),
            created_at: self.history.created_at.clone(),
        })
    }
}
