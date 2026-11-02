//! Thread creation — opening a fork, picking an id, depth bookkeeping.
//!
//! Thread *lifecycle* (active → merged → abandoned) lives in
//! [`super::mutations::ReasoningServer::set_thread_status`](super::ReasoningServer::set_thread_status); this file
//! owns the "create or extend a thread" side. The
//! `thread_depth_cache` makes repeated depth lookups O(1) once a note
//! is on a thread — invalidated only when a new thread is registered.

use std::collections::HashMap;

use crate::decision::domain::{Log, Note, Thread, ThreadStatus};

use super::core::ReasoningServer;

impl ReasoningServer {
    /// Restore lifecycle metadata and derive thread membership from canonical
    /// records. New imported questions obtain their first thread entry here.
    pub(crate) fn restored_threads(history: &Log) -> HashMap<String, Thread> {
        let mut threads: HashMap<String, Thread> = history
            .threads
            .iter()
            .flatten()
            .map(|thread| {
                let mut restored = thread.clone();
                // Canonical records own revisions and identity; member copies
                // are a view of those records.
                restored.notes.clear();
                (restored.id.clone(), restored)
            })
            .collect();
        for note in &history.notes {
            let Some(id) = &note.thread_id else {
                continue;
            };
            if !threads.contains_key(id) {
                let Some(from) = note.thread_from else {
                    continue;
                };
                let depth = history
                    .notes
                    .iter()
                    .find(|s| s.note == from)
                    .and_then(|s| s.thread_id.as_ref())
                    .and_then(|parent| threads.get(parent))
                    .map_or(1, |parent| parent.depth + 1);
                threads.insert(
                    id.clone(),
                    Thread {
                        id: id.clone(),
                        name: note
                            .thread_name
                            .clone()
                            .unwrap_or_else(|| "Unnamed question".into()),
                        from_note: from,
                        notes: Vec::new(),
                        status: ThreadStatus::Open,
                        created_at: note.timestamp.clone().unwrap_or_default(),
                        depth,
                        merged_into: None,
                    },
                );
            }
            if let Some(thread) = threads.get_mut(id) {
                if let Some(member) = thread.notes.iter_mut().find(|s| s.note == note.note) {
                    *member = note.clone();
                } else {
                    thread.notes.push(note.clone());
                }
            }
        }
        threads
    }

    pub fn calculate_thread_depth(&mut self, note_number: u32) -> u32 {
        if let Some(&v) = self.thread_depth_cache.get(&note_number) {
            return v;
        }
        let depth = if let Some(thread_id) = self.note_to_thread.get(&note_number) {
            self.threads
                .get(thread_id)
                .map(|b| b.depth + 1)
                .unwrap_or(1)
        } else {
            1
        };
        self.thread_depth_cache.insert(note_number, depth);
        depth
    }

    pub(crate) fn invalidate_thread_depth_cache(&mut self) {
        self.thread_depth_cache.clear();
    }

    /// Validate thread metadata without changing stored records. Continuing an
    /// existing thread needs only its id; every member retains the same origin
    /// and question so broadcasts and persisted notes describe the same path.
    pub(crate) fn resolve_thread(&self, note: &mut Note) -> Result<(), String> {
        if note.thread_from.is_none() && note.thread_id.is_none() {
            if note.thread_name.is_some() {
                return Err("A new thread needs thread_from and a question in thread_name".into());
            }
            return Ok(());
        }
        if !self.config.features.enable_threading {
            return Err("Threading is disabled in the project configuration".into());
        }
        if let Some(id) = note.thread_id.as_deref() {
            if id.trim().is_empty() {
                return Err("thread_id must not be blank".into());
            }
            if let Some(thread) = self.threads.get(id) {
                if note
                    .thread_from
                    .is_some_and(|from| from != thread.from_note)
                {
                    return Err(format!(
                        "Thread {id} began at note {}; keep that thread_from or omit it",
                        thread.from_note
                    ));
                }
                if note
                    .thread_name
                    .as_ref()
                    .is_some_and(|name| name != &thread.name)
                {
                    return Err(format!(
                        "Thread {id} is named {:?}; keep that thread_name or omit it",
                        thread.name
                    ));
                }
                note.thread_from = Some(thread.from_note);
                note.thread_name = Some(thread.name.clone());
                return Ok(());
            }
            if note.thread_from.is_none() {
                return Err(format!(
                    "Unknown thread_id {id:?}; use a returned thread id, or thread_from to start a new thread"
                ));
            }
        }
        let Some(from) = note.thread_from else {
            return Ok(());
        };
        if note
            .thread_name
            .as_ref()
            .is_some_and(|name| name.trim().is_empty())
        {
            return Err("thread_name must not be blank".into());
        }
        if from == note.note {
            let msg = format!("Cannot thread from self (note {})", note.note);
            tracing::warn!("{msg}");
            return Err(msg);
        }
        if !self.note_numbers.contains(&from) && self.archived_note(from).is_none() {
            let available = self.sorted_note_numbers();
            let msg = format!(
                "Cannot thread from note {from}: note does not exist. Available notes: {}",
                if available.is_empty() {
                    "none".into()
                } else {
                    available
                }
            );
            tracing::warn!("{msg}");
            return Err(msg);
        }

        let depth = self
            .note_to_thread
            .get(&from)
            .or_else(|| {
                self.archived_note(from)
                    .and_then(|note| note.thread_id.as_ref())
            })
            .and_then(|id| self.threads.get(id))
            .map_or(1, |thread| thread.depth + 1);
        if depth > self.config.system.max_thread_depth {
            return Err(format!(
                "Thread depth {depth} exceeds maximum {}",
                self.config.system.max_thread_depth
            ));
        }
        Ok(())
    }
}
