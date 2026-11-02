//! Note/log queries — point lookups and substring search.
//!
//! Read-only methods that find a single note (`note_by_number`,
//! `latest_revision_of`), iterate every recorded note (`all_notes`), or
//! produce structured search hits (`search_notes`).

use serde_json::{Value, json};

use crate::decision::domain::{NextAction, Note};
use crate::decision::util::text::excerpt_around;

use super::core::ReasoningServer;

impl ReasoningServer {
    /// Lookup a note by its note number, including any in threads.
    pub fn note_by_number(&self, n: u32) -> Option<Note> {
        if let Some(&i) = self.note_index.get(&n) {
            return self.history.notes.get(i).cloned();
        }
        for thread in self.threads.values() {
            if let Some(s) = thread.notes.iter().find(|s| s.note == n) {
                return Some(s.clone());
            }
        }
        self.project
            .logs
            .values()
            .flat_map(|history| &history.notes)
            .find(|note| note.note == n)
            .cloned()
    }

    /// Walk the `revised_by` chain forward from `n` and return the live
    /// (latest-revision) note. Returns the same note when nothing has revised
    /// it, and `None` when `n` doesn't exist. Capped at 256 hops so a
    /// cyclical chain can't pin a CPU.
    pub fn latest_revision_of(&self, n: u32) -> Option<Note> {
        let mut cur = self.note_by_number(n)?;
        let mut guard = 0u32;
        while let Some(next) = cur.revised_by {
            if guard >= 256 || next == cur.note {
                break;
            }
            guard += 1;
            match self.note_by_number(next) {
                Some(s) => cur = s,
                None => break,
            }
        }
        Some(cur)
    }

    /// Iterate every recorded note. Thread notes are also stored in
    /// `self.history.notes` (push happens unconditionally in `process_note`),
    /// so iterating main is sufficient — chaining threads in would double-count.
    pub(crate) fn all_notes(&self) -> impl Iterator<Item = &Note> {
        let selected = self.selected_key();
        self.history.notes.iter().chain(
            self.project
                .logs
                .iter()
                .filter(move |(key, _)| **key != selected)
                .flat_map(|(_, history)| &history.notes),
        )
    }

    /// Case-insensitive substring search across the text-bearing fields of
    /// every recorded note (main + threads). Returns at most `limit` matches
    /// in note-number order, each carrying a short excerpt and the field
    /// that matched.
    pub fn search_notes(&self, query: &str, limit: usize) -> Vec<Value> {
        let needle = query.trim().to_ascii_lowercase();
        if needle.is_empty() {
            return Vec::new();
        }

        let mut matches: Vec<(u32, Value)> = Vec::new();
        for note in self.all_notes() {
            let next_action_text = match &note.next_action {
                NextAction::Text(t) => t.clone(),
                NextAction::Structured(a) => {
                    let mut s = a.action.clone();
                    if let Some(tool) = &a.tool {
                        s.push_str(&format!(" [{tool}]"));
                    }
                    s
                }
            };

            let exec_ref = note.run_ref.as_deref().unwrap_or("");

            let fields: [(&str, &str); 7] = [
                ("thought", &note.thought),
                ("outcome", &note.outcome),
                ("context", &note.context),
                ("purpose", &note.purpose),
                ("rationale", &note.rationale),
                ("next_action", &next_action_text),
                ("run_ref", exec_ref),
            ];

            for (name, val) in &fields {
                let lower = val.to_ascii_lowercase();
                if let Some(pos) = lower.find(&needle) {
                    let excerpt = excerpt_around(val, pos, needle.len(), 60);
                    matches.push((
                        note.note,
                        json!({
                            "note": note.note,
                            "log_id": note.log_id,
                            "record_id": note.record_id,
                            "purpose": note.purpose,
                            "matched_field": name,
                            "excerpt": excerpt,
                            "thread_id": note.thread_id,
                            "confidence": note.confidence,
                            "revised_by": note.revised_by,
                        }),
                    ));
                    break;
                }
            }
        }

        matches.sort_by_key(|(n, _)| *n);
        matches.into_iter().map(|(_, v)| v).take(limit).collect()
    }
}
