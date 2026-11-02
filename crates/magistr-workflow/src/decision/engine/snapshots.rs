//! Read-only aggregations — every method that produces a JSON view of
//! engine state. None of these mutates; the engine guarantees they're
//! safe to call from inside response paths and from observer code.
//!
//! Includes `history_with_threads` (the internal builder used by
//! `thread_tree` and `export_history` in [`super::export`]) so all
//! "serialize the current state" code lives in one file.

use std::collections::HashSet;

use serde_json::{Map, Value, json};

use crate::decision::config::ROLLUP_PINNED_FLOOR;
use crate::decision::domain::{Log, Note, Thread, ThreadStatus};
use crate::decision::util::text::truncate_excerpt;

use super::core::ReasoningServer;

impl ReasoningServer {
    /// Clone the current history with threads folded into the
    /// `threads` field — the on-disk shape, suitable for export.
    /// Internal helper shared by `thread_tree`, `export_history`, and
    /// `history_snapshot`.
    pub(crate) fn history_with_threads(&self) -> Log {
        let mut snapshot = self.history.clone();
        let mut threads: Vec<Thread> = self.threads.values().cloned().collect();
        threads.sort_by(|a, b| a.from_note.cmp(&b.from_note).then(a.id.cmp(&b.id)));
        snapshot.threads = Some(threads);
        snapshot
    }

    /// JSON snapshot of the full history with threads resolved.
    pub fn history_snapshot(&self) -> Value {
        serde_json::to_value(self.history_with_threads()).unwrap_or(Value::Null)
    }

    /// Compact per-thread JSON snapshots, sorted by `from_note`.
    pub fn threads_snapshot(&self) -> Vec<Value> {
        let mut threads: Vec<Thread> = self.threads.values().cloned().collect();
        threads.sort_by(|a, b| a.from_note.cmp(&b.from_note).then(a.id.cmp(&b.id)));
        threads
            .into_iter()
            .filter_map(|b| serde_json::to_value(b).ok())
            .collect()
    }
    /// Complete log summaries, independent of the expiring display cache.
    pub fn logs_snapshot(&self) -> Vec<Value> {
        self.project
            .logs
            .values()
            .map(|history| {
                json!({
                    "log_id": history.log_id,
                    "note_count": history.notes.len(),
                    "completed": history.completed,
                    "created_at": history.created_at,
                    "updated_at": history.updated_at,
                })
            })
            .collect()
    }

    /// Count of pinned notes in the current window, excluding `exclude_note`.
    fn pinned_available(&self, exclude_note: Option<u32>) -> usize {
        self.pinned_order.len().saturating_sub(usize::from(
            exclude_note
                .and_then(|n| self.note_index.get(&n))
                .is_some_and(|index| self.history.notes[*index].pinned.unwrap_or(false)),
        ))
    }

    /// THE TWO-SIDED ROLLUP BUDGET, as `(pinned_budget, tail_budget)`.
    ///
    ///   ceiling       = max(rollup_max_items, ROLLUP_PINNED_FLOOR)
    ///   pinned_budget = min(pinned_available, max(FLOOR, ceiling - limit)), capped at ceiling
    ///   tail_budget   = min(limit, ceiling - pinned_budget)
    ///
    /// The CEILING is the configurable side; the FLOOR is a source constant and
    /// the ceiling is clamped up to it, so no configuration can satisfy the size
    /// limit by discarding the pinned conclusions the limit exists to protect.
    pub(crate) fn recent_notes_budget(
        &self,
        limit: usize,
        exclude_note: Option<u32>,
    ) -> (usize, usize) {
        let ceiling = self.config.system.rollup_max_items.max(ROLLUP_PINNED_FLOOR);
        let pinned_budget = self
            .pinned_available(exclude_note)
            .min(ceiling.saturating_sub(limit).max(ROLLUP_PINNED_FLOOR))
            .min(ceiling);
        let tail_budget = limit.min(ceiling - pinned_budget);
        (pinned_budget, tail_budget)
    }

    /// How many pinned notes the rollup dropped to stay inside its ceiling.
    /// Zero means the rollup is complete and the response says nothing — a
    /// short log pays nothing for a budget it never hits.
    pub fn pinned_withheld_count(&self, limit: usize, exclude_note: Option<u32>) -> usize {
        let (pinned_budget, _) = self.recent_notes_budget(limit, exclude_note);
        self.pinned_available(exclude_note)
            .saturating_sub(pinned_budget)
    }

    /// Last `limit` notes from main history, in chronological order, with
    /// optional exclusion of a single note number (used by the
    /// response-enrichment path to avoid echoing the just-added note).
    ///
    /// Pinned notes are folded in regardless of their chronological position
    /// so that load-bearing conclusions stay visible after the log grows
    /// past the window — but only up to the budget, newest-first. See
    /// `Self::recent_notes_budget` for the arithmetic and
    /// [`Self::pinned_withheld_count`] for what the response says about the
    /// ones it dropped.
    pub fn recent_notes_rollup(&self, limit: usize, exclude_note: Option<u32>) -> Vec<Value> {
        let (pinned_budget, tail_budget) = self.recent_notes_budget(limit, exclude_note);

        let mut selected: Vec<&Note> = Vec::new();
        let mut selected_nums: HashSet<u32> = HashSet::new();

        // First pass: the NEWEST `pinned_budget` pinned notes. Walking in
        // reverse takes the newest; before the budget existed this pass walked
        // forward and took every pinned note unconditionally, which is what
        // made `limit` a cap on the tail only.
        for number in self.pinned_order.iter().rev() {
            let Some(note) = self
                .note_index
                .get(number)
                .and_then(|index| self.history.notes.get(*index))
            else {
                continue;
            };
            if selected.len() >= pinned_budget {
                break;
            }
            if exclude_note == Some(note.note) {
                continue;
            }
            if note.pinned.unwrap_or(false) && selected_nums.insert(note.note) {
                selected.push(note);
            }
        }

        // Second pass: walk recent-first and fill the unpinned tail.
        let ceiling = pinned_budget + tail_budget;
        for note in self.history.notes.iter().rev() {
            if selected.len() >= ceiling {
                break;
            }
            if exclude_note == Some(note.note) {
                continue;
            }
            if selected_nums.insert(note.note) {
                selected.push(note);
            }
        }

        // Render in note-number order so the rollup reads chronologically.
        // Null/false fields are omitted so the rollup stays tight at depth.
        // `rationale_excerpt` captures the WHY of each prior note — most
        // valuable when navigating mid-log, currently invisible after a
        // note falls out of the model's own context.
        selected.sort_by_key(|s| s.note);
        selected
            .into_iter()
            .map(|s| {
                let mut obj = Map::new();
                obj.insert("n".into(), json!(s.note));
                obj.insert("purpose".into(), json!(s.purpose));
                obj.insert(
                    "thought_excerpt".into(),
                    json!(truncate_excerpt(&s.thought, 80)),
                );
                if !s.rationale.trim().is_empty() {
                    obj.insert(
                        "rationale_excerpt".into(),
                        json!(truncate_excerpt(&s.rationale, 60)),
                    );
                }
                if let Some(c) = s.confidence {
                    obj.insert("confidence".into(), json!(c));
                }
                if let Some(r) = s.revised_by {
                    obj.insert("revised_by".into(), json!(r));
                }
                if let Some(b) = &s.thread_id {
                    obj.insert("thread_id".into(), json!(b));
                }
                if s.pinned.unwrap_or(false) {
                    obj.insert("pinned".into(), json!(true));
                }
                Value::Object(obj)
            })
            .collect()
    }

    /// Compact thread overview suitable for inclusion in note responses.
    pub fn threads_summary(&self) -> Vec<Value> {
        let mut v: Vec<&Thread> = self.threads.values().collect();
        v.sort_by(|a, b| a.from_note.cmp(&b.from_note).then(a.id.cmp(&b.id)));
        v.into_iter()
            .map(|b| {
                json!({
                    "id": b.id,
                    "name": b.name,
                    "from_note": b.from_note,
                    "notes": b.notes.len(),
                    "status": match b.status {
                        ThreadStatus::Open => "open",
                        ThreadStatus::Merged => "merged",
                        ThreadStatus::Abandoned => "abandoned",
                    },
                    "depth": b.depth,
                })
            })
            .collect()
    }

    /// Log-wide metacognitive snapshot. Aggregates per-note warnings into
    /// patterns that only show up when you look across the whole history:
    /// open hypotheses with no defenders, stale threads that nobody closed,
    /// confidence drift, dependencies hanging off a revised note.
    pub fn checkpoint_snapshot(&self) -> Value {
        // open_hypotheses: purpose=hypothesis with no downstream note whose
        // purpose is validation or correction.
        let mut open_hypotheses: Vec<Value> = Vec::new();
        for note in &self.history.notes {
            if !note.purpose.eq_ignore_ascii_case("hypothesis") {
                continue;
            }
            let dependents = self.direct_dependents(note.note);
            let has_check = dependents.iter().any(|n| {
                self.note_by_number(*n)
                    .map(|s| {
                        matches!(
                            s.purpose.to_ascii_lowercase().as_str(),
                            "validation" | "correction"
                        )
                    })
                    .unwrap_or(false)
            });
            if !has_check {
                open_hypotheses.push(json!({
                    "note": note.note,
                    "thought_excerpt": truncate_excerpt(&note.thought, 80),
                    "confidence": note.confidence,
                }));
            }
        }

        // stale_threads: active threads whose latest note is more than
        // (max_history_size / 4) notes behind the latest main note.
        let stale_threshold = (self.config.system.max_history_size / 4).max(2) as u32;
        let head = self.history.notes.last().map(|s| s.note).unwrap_or(0);
        let mut stale_threads: Vec<Value> = Vec::new();
        for thread in self.threads.values() {
            if !matches!(thread.status, ThreadStatus::Open) {
                continue;
            }
            let last_note = thread
                .notes
                .iter()
                .map(|s| s.note)
                .max()
                .unwrap_or(thread.from_note);
            if head.saturating_sub(last_note) >= stale_threshold {
                stale_threads.push(json!({
                    "id": thread.id,
                    "name": thread.name,
                    "last_note": last_note,
                    "notes_behind": head.saturating_sub(last_note),
                }));
            }
        }

        // confidence_trend: slope of the last 5 confidences. Reported as a
        // qualitative label so the model can react without doing math.
        let recent_conf: Vec<f64> = self
            .history
            .notes
            .iter()
            .rev()
            .filter_map(|s| s.confidence)
            .take(5)
            .collect();
        let trend = if recent_conf.len() < 2 {
            "insufficient_data"
        } else {
            // recent_conf is in reverse order — newest first. Compute slope
            // from oldest to newest by walking from the end.
            let xs: Vec<f64> = (0..recent_conf.len()).map(|i| i as f64).collect();
            let ys: Vec<f64> = recent_conf.iter().rev().copied().collect();
            let n = xs.len() as f64;
            let sx: f64 = xs.iter().sum();
            let sy: f64 = ys.iter().sum();
            let sxy: f64 = xs.iter().zip(&ys).map(|(x, y)| x * y).sum();
            let sxx: f64 = xs.iter().map(|x| x * x).sum();
            let denom = n * sxx - sx * sx;
            let slope = if denom == 0.0 {
                0.0
            } else {
                (n * sxy - sx * sy) / denom
            };
            if slope > 0.05 {
                "rising"
            } else if slope < -0.05 {
                "falling"
            } else {
                "stable"
            }
        };

        // revised_but_undefended: notes that have been revised AND have at
        // least one dependent whose note number is GREATER than the revising
        // note's number but did NOT explicitly cite the revising note. That
        // dependent built on the older form without acknowledging the update.
        let mut revised_undefended: Vec<Value> = Vec::new();
        for note in &self.history.notes {
            let Some(rev_by) = note.revised_by else {
                continue;
            };
            let undefended: Vec<u32> = self
                .all_notes()
                .filter(|s| s.note > rev_by)
                .filter(|s| {
                    s.dependencies.as_ref().is_some_and(|d| {
                        d.iter().any(|e| e.note() == note.note)
                            && !d.iter().any(|e| e.note() == rev_by)
                    })
                })
                .map(|s| s.note)
                .collect();
            if !undefended.is_empty() {
                revised_undefended.push(json!({
                    "note": note.note,
                    "revised_by": rev_by,
                    "depending_notes_unaware": undefended,
                }));
            }
        }

        // refuted_chain_alerts: notes where any transitive upstream dep has
        // been refuted somewhere in the log.
        let mut refuted_alerts: Vec<Value> = Vec::new();
        for note in &self.history.notes {
            let upstream = self.walk_deps(note.note, 256);
            let mut refuted_in_chain: Vec<u32> = Vec::new();
            for anc in &upstream {
                if !self.refuters_of(*anc, /* exclude */ note.note).is_empty() {
                    refuted_in_chain.push(*anc);
                }
            }
            if !refuted_in_chain.is_empty() {
                refuted_alerts.push(json!({
                    "note": note.note,
                    "refuted_ancestors": refuted_in_chain,
                }));
            }
        }

        json!({
            "open_hypotheses": open_hypotheses,
            "stale_threads": stale_threads,
            "confidence_trend": trend,
            "revised_but_undefended": revised_undefended,
            "refuted_chain_alerts": refuted_alerts,
            // Maintenance health. The other five findings are about the SHAPE of
            // the reasoning; this one is about the shape of the memory holding
            // it — the pinned set's size and how hard it is pressing on the
            // rollup budget. It reports and proposes; it never unpins. See
            // [`super::maintenance`] for the policy.
            "maintenance": serde_json::to_value(self.maintenance_report())
                .unwrap_or(Value::Null),
        })
    }

    /// All currently-pinned notes in note-number order. Returns compact
    /// note descriptors suitable for an "anchors" view. Mirrors
    /// `recent_notes_rollup`'s null-omission policy.
    pub fn pinned_notes(&self) -> Vec<Value> {
        let mut pinned: Vec<&Note> = self
            .history
            .notes
            .iter()
            .filter(|s| s.pinned.unwrap_or(false))
            .collect();
        pinned.sort_by_key(|s| s.note);
        pinned
            .into_iter()
            .map(|s| {
                let mut obj = Map::new();
                obj.insert("note".into(), json!(s.note));
                obj.insert("purpose".into(), json!(s.purpose));
                obj.insert(
                    "thought_excerpt".into(),
                    json!(truncate_excerpt(&s.thought, 120)),
                );
                obj.insert(
                    "outcome_excerpt".into(),
                    json!(truncate_excerpt(&s.outcome, 120)),
                );
                if let Some(c) = s.confidence {
                    obj.insert("confidence".into(), json!(c));
                }
                if let Some(r) = s.revised_by {
                    obj.insert("revised_by".into(), json!(r));
                }
                if let Some(b) = &s.thread_id {
                    obj.insert("thread_id".into(), json!(b));
                }
                Value::Object(obj)
            })
            .collect()
    }

    /// Runtime introspection: the high-level state the model needs to know
    /// to decide whether the log is durable, how big it is, and where it
    /// lives on disk. `verbose` folds in the formerly-separate `pinned` and
    /// `logs` listings.
    pub fn status_snapshot(&self, verbose: bool) -> Value {
        let data_dir = self.config.persistence.data_dir.display().to_string();
        let mut obj = Map::new();
        obj.insert(
            "persistence_enabled".into(),
            json!(self.config.persistence.enabled),
        );
        obj.insert("data_dir".into(), json!(data_dir));
        obj.insert(
            "logs_enabled".into(),
            json!(self.config.features.enable_logs),
        );
        // Surface the configured default log_id so an operator can tell
        // at a glance whether MAGISTR_WORKFLOW_AUTO_LOG / MAGISTR_WORKFLOW_DEFAULT_LOG_ID
        // actually took effect in this process. Without this, a stale
        // env-var setup looks identical to a working one from the outside.
        obj.insert(
            "default_log_id".into(),
            json!(self.config.features.default_log_id),
        );
        obj.insert("active_log".into(), json!(self.active_log));
        obj.insert("logs_count".into(), json!(self.project.logs.len()));
        obj.insert(
            "log_present".into(),
            json!(self.project.logs.contains_key(&self.selected_key())),
        );
        obj.insert("total_notes".into(), json!(self.history.notes.len()));
        // THE NEXT SAFE NOTE NUMBER — the one field decision_record requires
        // and, until this shipped, the one field with no authoritative source.
        // `total_notes` above is a COUNT of retained notes and the numbering is
        // sparse, so it is off by however many notes have been trimmed or
        // written out of order; a log that numbered from it wrote an orphan
        // 846 below the head (note 577 left in the store as the evidence).
        // Deliberately the SAME expression `process_note` uses to
        // auto-assign an omitted number, so the number we publish and the number
        // a caller actually gets cannot drift apart.
        obj.insert(
            "next_note_number".into(),
            json!(self.max_note_number_in_project().saturating_add(1)),
        );
        obj.insert("threads_count".into(), json!(self.threads.len()));
        obj.insert(
            "pinned_count".into(),
            json!(
                self.history
                    .notes
                    .iter()
                    .filter(|s| s.pinned.unwrap_or(false))
                    .count()
            ),
        );
        obj.insert("completed".into(), json!(self.history.completed));
        obj.insert(
            "recent_notes_limit".into(),
            json!(self.config.system.recent_notes_limit),
        );
        obj.insert(
            "max_history_size".into(),
            json!(self.config.system.max_history_size),
        );
        obj.insert(
            "strict_mode".into(),
            json!(self.config.validation.strict_mode),
        );
        obj.insert("version".into(), json!(env!("CARGO_PKG_VERSION")));
        if verbose {
            obj.insert("pinned".into(), Value::Array(self.pinned_notes()));
            obj.insert("logs".into(), Value::Array(self.logs_snapshot()));
        }
        Value::Object(obj)
    }
}
