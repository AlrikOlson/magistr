//! `process_note_native` and its tightly-coupled helpers.
//!
//! `process_note_native` is the biggest single method on `ReasoningServer` —
//! it's the load-bearing entry point for `decision_record`.
//! Recovery, validation, log lifecycle, threading, revisions, and
//! response enrichment all flow through it. The supporting helpers
//! that live alongside it here:
//!
//! * `Self::compute_note_warnings` — soft advisories surfaced in the
//!   note's response.
//! * `Self::make_error` — typed `NoteError` builder.
//! * `Self::trim_history` — bounded-history sliding window.
//!
//! These methods are kept together because they share private
//! mutable state and a lot of internal coupling. The rest of the
//! engine's methods live in concern-focused sibling modules
//! ([`super::validation`], [`super::threading`], [`super::logs`],
//! [`super::revisions`], [`super::numbering`], [`super::lookup`],
//! [`super::impact`], [`super::snapshots`], [`super::mutations`],
//! [`super::export`], [`super::recovery`]).

use std::collections::HashSet;

use serde_json::{Map, Value, json};

use crate::decision::broadcast::BroadcastFrame;
use crate::decision::config::namespace_log_id;
use crate::decision::constants::LOW_CONFIDENCE_THRESHOLD;
use crate::decision::domain::Note;
use crate::decision::util::text::truncate_excerpt;

use super::core::ReasoningServer;
use crate::decision::domain::{NoteError, RecordedNote};

impl ReasoningServer {
    pub(crate) fn trim_history(&mut self) {
        let max = self.config.system.max_history_size;
        if self.history.notes.len() <= max {
            return;
        }
        let drop_count = self.history.notes.len() - max;
        let removed_note_numbers: HashSet<u32> = self.history.notes[..drop_count]
            .iter()
            .map(|s| s.note)
            .collect();
        self.history.notes.drain(..drop_count);
        self.pinned_order
            .retain(|number| !removed_note_numbers.contains(number));

        for n in &removed_note_numbers {
            self.note_index.remove(n);
            self.note_numbers.remove(n);
            self.note_to_thread.remove(n);
        }
        // Rebuild note_index because indexes shifted after drain.
        self.note_index.clear();
        for (idx, s) in self.history.notes.iter().enumerate() {
            self.note_index.insert(s.note, idx);
        }

        // The retained note window is not the lifetime of an investigation.
        // Keep its origin and status so id-only continuation still works after
        // that origin leaves the window; only evict old member copies.
        for thread in self.threads.values_mut() {
            thread
                .notes
                .retain(|note| !removed_note_numbers.contains(&note.note));
        }

        self.invalidate_thread_depth_cache();
        tracing::debug!(
            "📋 History trimmed to {} notes (removed {} old notes)",
            max,
            removed_note_numbers.len()
        );
    }

    pub fn process_note_native(&mut self, mut note: Note) -> Result<RecordedNote, NoteError> {
        let selected = if self.config.features.enable_logs {
            let raw = note
                .log_id
                .as_deref()
                .filter(|id| !id.trim().is_empty())
                .or(self.config.features.default_log_id.as_deref())
                .unwrap_or(&self.project_id);
            let id = namespace_log_id(&self.project_id, raw);
            self.persistence.validate_log_id(&id).map_err(|error| {
                NoteError::rejected(error.to_string(), "Choose a valid log name.")
            })?;
            note.log_id = Some(id.clone());
            Some(id)
        } else {
            None
        };
        let record_id = uuid::Uuid::new_v4().to_string();
        note.record_id = Some(record_id.clone());
        let committed = self.transact_append(selected, note).map_err(|mut error| {
            error.record_id = Some(record_id);
            error
        })?;
        let mut recorded = committed.value;
        recorded.source = committed.source;
        recorded.report["source"] = json!(committed.source);
        recorded.report["record_id"] = json!(recorded.note.record_id);
        self.cleanup_expired_logs(false);
        if let Some(broadcaster) = &self.broadcaster {
            broadcaster.emit(BroadcastFrame::NoteAppended {
                log_id: self.active_log.clone(),
                note: Box::new(recorded.note.clone()),
            });
            if let Some(revised_note) = recorded.note.revises_note {
                broadcaster.emit(BroadcastFrame::NoteRevised {
                    log_id: self.active_log.clone(),
                    revised_note,
                    by_note: recorded.note.note,
                });
            }
        }
        self.mirror_note_to_repo(&recorded.note);
        self.cloud_push_note(&recorded.note);
        Ok(recorded)
    }

    pub(crate) fn append_report(&self, prepared: &super::append::PreparedAppend) -> RecordedNote {
        let note = &prepared.note;
        let recorded_note = note.clone();
        let auto_numbered = prepared.auto_numbered;
        let recovered_fields = &prepared.recovered_fields;
        let tools_used = &prepared.tools_used;
        let note_number = note.note;
        let confidence = note.confidence;
        let revises_note = note.revises_note;
        let thread_id = note.thread_id.clone();
        let thread_name = note.thread_name.clone();
        let thread_from = note.thread_from;
        let estimated_total = note.estimated_total;
        let low_confidence = confidence
            .map(|c| c < LOW_CONFIDENCE_THRESHOLD)
            .unwrap_or(false);
        let uncertainty_notes = note.uncertainty_notes.clone();

        let formatted = self.format_output(note);
        tracing::debug!("{formatted}");
        if low_confidence {
            let pct = (confidence.unwrap_or(0.0) * 100.0).round() as i32;
            tracing::debug!(
                "⚠️ Low confidence ({pct}%): {}",
                uncertainty_notes
                    .as_deref()
                    .unwrap_or("Consider verification")
            );
        }

        // Response is intentionally tight — the caller already has the note
        // it sent in its own context, so we don't echo `next_action`,
        // top-level `confidence`, or `completed: false` (those would be
        // pure noise). We DO surface anything the server computed or
        // derived: note counts, thread id (auto-assigned), warnings,
        // recent_notes with state the caller couldn't easily reconstruct.
        let mut response: Map<String, Value> = Map::new();
        response.insert("note".into(), json!(note_number));
        response.insert("estimated_total".into(), json!(estimated_total));
        response.insert("total_notes".into(), json!(self.history.notes.len()));
        if self.history.completed {
            response.insert("completed".into(), json!(true));
        }
        if let Some(r) = revises_note {
            response.insert("revised_note".into(), json!(r));
        }
        if let Some(id) = thread_id {
            let mut thread_obj = Map::new();
            thread_obj.insert("id".into(), json!(id));
            if let Some(name) = thread_name {
                thread_obj.insert("name".into(), json!(name));
            }
            if let Some(from) = thread_from {
                thread_obj.insert("from".into(), json!(from));
            }
            response.insert("thread".into(), Value::Object(thread_obj));
        }

        // ─── Response enrichments (see also `recent_notes_rollup`, `threads_summary`). ───
        // These give the model the context it needs to self-orient without
        // making a second call. Each field is added only when non-trivial so
        // we don't bloat token usage on simple logs.

        // Echo a short excerpt of what was just recorded — confirms the note
        // landed and gives the model a stable anchor it can refer to later.
        let last_idx = self.history.notes.len().saturating_sub(1);
        if let Some(stored) = self.history.notes.get(last_idx) {
            response.insert(
                "thought_excerpt".into(),
                json!(truncate_excerpt(&stored.thought, 120)),
            );
            response.insert(
                "outcome_excerpt".into(),
                json!(truncate_excerpt(&stored.outcome, 120)),
            );

            // Acknowledge optional inputs the model declared — if the model
            // sees `dependencies` or `tools_recorded` echoed, it knows the
            // server accepted them.
            if let Some(deps) = &stored.dependencies {
                if !deps.is_empty() {
                    response.insert("dependencies".into(), json!(deps));
                }
            }
        }
        if !tools_used.is_empty() {
            response.insert("tools_recorded".into(), json!(tools_used));
        }

        // Warnings: never an error, but flag situations the model should know
        // about (e.g. depending on prior reasoning that itself had low confidence).
        let mut warnings = self.compute_note_warnings(note_number, confidence);
        if let Some(requested) = auto_numbered {
            let how = if requested == 0 {
                "note was omitted".to_string()
            } else {
                format!("requested note {requested} was already taken")
            };
            warnings.insert(
                0,
                format!(
                    "Auto-assigned note {note_number} ({how}). Note numbers are \
                     unique project-wide; use this number for back-references."
                ),
            );
        }
        if !recovered_fields.is_empty() {
            warnings.insert(
                0,
                format!(
                    "Auto-recovered field(s) {recovered:?} from XML-injected text in your \
                     parameter values. Cause: literal `</parameter>` or `</thought>` inside \
                     a value closes that parameter early in the harness's tool-call parser. \
                     The siblings you intended to send were embedded as text in `thought` \
                     and we extracted them. Next time, describe markup in prose or fence it \
                     in backticks so the literal characters don't appear in any value.",
                    recovered = recovered_fields,
                ),
            );
        }
        if !warnings.is_empty() {
            response.insert("warnings".into(), json!(warnings));
        }

        // A rolling view of the last few prior notes — purpose, ~80-char
        // thought excerpt, confidence. Lets the model self-orient mid-log
        // without having to call `decision_export`. Pinned notes are
        // promoted into this window so load-bearing conclusions don't fall
        // out as the log lengthens.
        let limit = self.config.system.recent_notes_limit;
        let recent = self.recent_notes_rollup(limit, Some(note_number));
        if !recent.is_empty() {
            response.insert("recent_notes".into(), Value::Array(recent));
        }

        // SUMMARY-PLUS-HANDLE. When the budget dropped pinned notes, say so and
        // name the tool that fetches them. The loss then becomes DELIBERATE:
        // an agent that needs an older pinned conclusion is told it exists and
        // how to get it, instead of never learning it was there. Emitted only
        // when something was actually withheld, so short logs pay nothing.
        let withheld = self.pinned_withheld_count(limit, Some(note_number));
        if withheld > 0 {
            response.insert(
                "recent_notes_omitted".into(),
                json!({
                    "pinned_withheld": withheld,
                    "fetch_with": "decision_get",
                    "note": "Older pinned notes were withheld to keep this response inside its size \
                             budget. They are still in the log — fetch any of them by note_number \
                             with decision_get, or list them with decision_status(verbose: true).",
                }),
            );
        }

        // Thread overview is always relevant when any threads exist, not
        // just on the note that created one.
        let threads_summary = self.threads_summary();
        if !threads_summary.is_empty() {
            response.insert("threads_summary".into(), Value::Array(threads_summary));
        }

        RecordedNote {
            project_id: self.project_id.clone(),
            note: recorded_note,
            source: Self::memory_source(),
            report: Value::Object(response),
        }
    }

    /// Mirror the most-recently-recorded note into the git-native log
    /// as an Agent Trace JSONL record. No-op when no `repo_sink`
    /// is attached. Fire-and-forget: every failure is logged at WARN and
    /// dropped so the `process_note_native` path is never affected. The log is
    /// committed when the note closes the log (`is_final_note`) and records
    /// are `shared`.
    fn mirror_note_to_repo(&self, stored: &Note) {
        let Some(mirror) = &self.mirror else {
            return;
        };
        // One JSONL file per log; fall back to the project id when no
        // named log is active so default-history notes still land somewhere.
        let log_id = self
            .active_log
            .clone()
            .unwrap_or_else(|| self.project_id.clone());

        let payload = serde_json::to_value(stored).unwrap_or(Value::Null);
        // Commit on log close, but only for the shared (committed)
        // partition — the local partition is gitignored. Hand off to the worker
        // thread; building the record (which shells git), the append, and any
        // commit all happen off this engine's lock.
        let closes = self.repo_shared && stored.is_final_note == Some(true);
        mirror.submit(crate::infra::MirrorJob {
            family: "decision",
            kind: "note",
            session_id: log_id,
            shared: self.repo_shared,
            payload,
            files: vec![], // Reasoning notes attribute no code.
            closes,
        });
    }

    /// Soft, non-fatal advisories about the just-recorded note.
    /// Warning text is kept compact — the response is already keyed by
    /// `note`, so the redundant "Note N ..." prefix is dropped.
    fn compute_note_warnings(&self, note_number: u32, confidence: Option<f64>) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let last_idx = self.history.notes.len().saturating_sub(1);
        let Some(stored) = self.history.notes.get(last_idx) else {
            return out;
        };

        if stored.log_id.is_some() && !self.config.features.enable_logs {
            out.push(
                "log_id ignored — logs are disabled (set MAGISTR_WORKFLOW_ENABLE_LOGS=true). Note landed in the default history."
                    .into(),
            );
        }

        if let Some(deps) = &stored.dependencies {
            for edge in deps {
                let dep_n = edge.note();
                let relation = edge.relation();
                // Skip prior-confidence and revised-by checks for explicitly
                // "refutes" edges — refuting evidence is healthy and not a
                // shaky-dependency signal.
                let is_refutes = relation == Some("refutes");
                if let Some(&i) = self.note_index.get(&dep_n) {
                    if let Some(prior) = self.history.notes.get(i) {
                        if !is_refutes {
                            if let Some(prior_c) = prior.confidence {
                                if prior_c < LOW_CONFIDENCE_THRESHOLD {
                                    out.push(format!(
                                        "dep on note {dep_n} has low confidence ({}%) — validate before building",
                                        (prior_c * 100.0).round() as i32
                                    ));
                                }
                            }
                            if let Some(revised_by) = prior.revised_by {
                                out.push(format!(
                                    "dep on note {dep_n} was revised by note {revised_by} — re-check it still holds"
                                ));
                            }
                        }
                        // Refuted-prior advisory: this note depends on N
                        // (supportively / unlabeled) but some other note
                        // refutes N. Fire only for non-refutes edges.
                        if !is_refutes {
                            let refuted_by =
                                self.refuters_of(dep_n, /* exclude */ stored.note);
                            if !refuted_by.is_empty() {
                                let joined = refuted_by
                                    .iter()
                                    .map(|n| n.to_string())
                                    .collect::<Vec<_>>()
                                    .join(", ");
                                out.push(format!(
                                    "dep on note {dep_n} is refuted by note(s) {joined} — re-validate before relying on it"
                                ));
                            }
                        }
                    }
                }
            }
        }

        if let Some(c) = confidence {
            if c < LOW_CONFIDENCE_THRESHOLD && stored.uncertainty_notes.is_none() {
                out.push(format!(
                    "confidence is low ({}%) but no uncertainty_notes — record what's uncertain",
                    (c * 100.0).round() as i32
                ));
            }
        }

        if note_number > stored.estimated_total {
            out.push(format!(
                "note {note_number} exceeds estimated_total {} — call decision_revise_estimate",
                stored.estimated_total
            ));
        }

        out
    }

    pub(crate) fn make_error(&self, msg: &str) -> NoteError {
        let hint = if self.config.validation.strict_mode {
            "Strict mode is enabled. Set MAGISTR_WORKFLOW_STRICT_MODE=false for flexible validation."
        } else {
            "Check that all required fields are provided."
        };
        NoteError::rejected(msg, hint)
    }
}

#[cfg(test)]
#[path = "process.tests.rs"]
mod tests;
