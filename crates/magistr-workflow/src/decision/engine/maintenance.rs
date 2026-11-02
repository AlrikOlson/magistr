//! Memory MAINTENANCE — and the one quantity in this engine that nothing bounds.
//!
//! Three layers of this system already forget, and every one of them is bounded
//! or deliberately lossless. Exactly one is monotone, and it is the subject of
//! this module:
//!
//! | Layer | Who forgets | Bound |
//! |---|---|---|
//! | the rollup | [`super::snapshots`]'s `recent_notes_rollup` | ceiling `rollup_max_items`, floor [`ROLLUP_PINNED_FLOOR`](crate::decision::config::ROLLUP_PINNED_FLOOR) |
//! | the window | `trim_history`, on every recorded note | `max_history_size` |
//! | the store | **nobody, deliberately** | none — [`crate::decision::persistence`] merges on save, so the archive is the durable record and may exceed the window |
//! | the pin bit | **NOBODY** | **none** |
//!
//! [`pin`](crate::decision::engine::ReasoningServer::pin)-style pinning is a one-way door: something sets
//! the bit and nothing ever clears it. Every pin then competes for the same
//! handful of slots in the rollup's pinned budget, forever. At the numbers that
//! motivated this module — 303 pinned notes against a pinned budget of 9 — 294
//! pinned conclusions are withheld from every single `decision_record`, and
//! the ones that survive are simply the newest, not the useful.
//!
//! So maintenance here is **pin-bit hygiene, and nothing else**.
//!
//! # What ages out, what summarizes, what unpins, what is never touched
//!
//! | Question | Answer |
//! |---|---|
//! | What ages out of rollups? | Nothing new. The two-sided budget already decides that, newest-pinned-first. |
//! | What summarizes? | **Nothing.** Compaction is rejected below. |
//! | What unpins? | Only an explicit `decision_pin(pinned: false)` call. This module never clears a bit. |
//! | What is never touched? | Any note; the store; a pin inside the budget; a pin anything still depends on; a pin on either side of a revision. |
//!
//! # The decay rule, as arithmetic
//!
//! A pinned note is a DECAY CANDIDATE when ALL of:
//!
//! 1. it is pinned;
//! 2. its index in the newest-first pinned ordering is `>= pinned_budget +`
//!    [`PIN_DECAY_LAG`] — not merely withheld from today's rollup, but withheld
//!    with room to spare, so a one-item flex in `recent_notes_limit` cannot turn
//!    yesterday's candidate into today's mistake;
//! 3. nothing in the retained window depends on it;
//! 4. it neither revises another note nor has been revised — a correction is the
//!    one class where losing the newer form actively misleads.
//!
//! Conditions 3 and 4 are the never-touched list, and they exist to make the
//! proposal SAFE TO ACCEPT. A proposal a human would be wrong to accept is worse
//! than no proposal, because it spends the only thing an advisory channel has.
//!
//! Decay is measured in the budget's own units rather than in notes or logs:
//! elapsed time is a proxy for "has this stopped being used", and the budget
//! answers that question directly and exactly.
//!
//! # Propose, never act
//!
//! [`ReasoningServer::maintenance_report`] COMPUTES; it does not mutate. It is a
//! `&self` method by design and returns a report that names the disposal verb.
//! That verb is the *existing* `decision_pin(pinned: false)` — this module
//! adds no second way to unpin, because two paths to the same mutation is how
//! the rule drifts on one of them. The contract is `plan_reprioritize`'s
//! exactly: the tool proposes, the human disposes, the state never moves alone.
//!
//! # Rejected alternatives
//!
//! **COMPACTION** — folding a plan task's open/decision/close trio into one
//! summary note. Rejected twice over. *Authorship*: either the agent writes the
//! summary,
//! in which case it is just recording a note and needs no mechanism, or the
//! SERVER writes it, in which case the server authors reasoning it did not do
//! and the log stops being a record of what was thought. *Arithmetic*: a
//! compaction ADDS a note to the store while claiming to reduce, and the pin on
//! the new summary competes for the same budget as the three it replaced.
//!
//! **LOG-BOUNDARY DEMOTION.** There is no boundary to hang it off:
//! `generate_auto_log_id` resolves to a stable per-project id on purpose, so
//! a project has ONE log across every conversation it will ever have. And a
//! boundary-triggered demote would be silent, which is the one thing this
//! module's contract forbids.
//!
//! **EVICTION**, which is where the 2026 field converged — memory eviction,
//! consolidation layers that "keep, merge, or evict", memory-governance
//! primitives. Every one of them deletes or rewrites the record. We take the
//! priority half and refuse the record half, because this store is an audit
//! trail of reasoning that actually happened, not a cache of facts. Unpinning is
//! not deleting, and that distinction is the whole design.
//!
//! **A NEW TOOL** (`decision_maintenance` / `decision_decay`). The report is a
//! read-only metacognitive view and `decision_review` is already that
//! tool; tool surface is charged per call to every agent that lists it.

use schemars::JsonSchema;
use serde::Serialize;
use std::collections::HashSet;

use crate::decision::config::PIN_DECAY_LAG;
use crate::decision::domain::Note;
use crate::decision::util::text::truncate_excerpt;

use super::core::ReasoningServer;

/// How many decay candidates the report names before it starts counting instead
/// of listing.
///
/// A report that listed all 294 withheld pins would reproduce, on the checkpoint
/// tool, the exact cost failure the rollup budget exists to fix. `total` is
/// always reported alongside, so the truncation is summary-plus-handle rather
/// than a silent cut.
pub const MAINTENANCE_CANDIDATE_CAP: usize = 10;

/// How hard the pinned set is pressing on the rollup budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PinPressure {
    /// Every pinned note fits in the budget. Nothing to do.
    None,
    /// Pins are being withheld, but none of them have gone stale by the decay
    /// rule — the log is simply busy.
    Building,
    /// Pins are being withheld AND some of them have stopped earning their slot.
    Saturated,
}

/// One pinned note that has stopped earning its place in the rollup budget.
///
/// A candidate is a PROPOSAL. Nothing in this crate acts on it.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct DecayCandidate {
    pub note: u32,
    pub purpose: String,
    pub outcome_excerpt: String,
    /// How many pinned notes are newer than this one.
    pub pinned_behind: usize,
}

/// Maintenance health of the log: how big the pinned set is, how hard it is
/// pressing on the rollup budget, and which pins could be released.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct MaintenanceReport {
    /// Pinned notes in the retained window.
    pub pinned_total: usize,
    /// How many of them a rollup can actually carry at the current config.
    pub pinned_in_budget: usize,
    /// How many are withheld from every rollup: `pinned_total - pinned_in_budget`.
    pub pinned_withheld: usize,
    /// The rollup's total item ceiling, pinned and unpinned together.
    pub rollup_ceiling: usize,
    pub pressure: PinPressure,
    /// Candidates by the decay rule, newest-first, capped at
    /// [`MAINTENANCE_CANDIDATE_CAP`].
    pub decay_candidates: Vec<DecayCandidate>,
    /// How many candidates exist in total — `decay_candidates` may be shorter.
    pub decay_candidates_total: usize,
    /// The one verb that disposes of a candidate. Always present, so the report
    /// can never read as something that already happened.
    pub unpin_with: &'static str,
}

impl ReasoningServer {
    /// Maintenance health of the log. Pure: reads the history, mutates
    /// nothing, and in particular never clears a pin — see the module doc.
    pub fn maintenance_report(&self) -> MaintenanceReport {
        let limit = self.config.system.recent_notes_limit;
        let (pinned_budget, tail_budget) = self.recent_notes_budget(limit, None);

        // Newest-first, the same ordering the rollup's pinned pass walks — the
        // report must agree with the budget it is reporting on.
        let mut pinned: Vec<&Note> = self
            .history
            .notes
            .iter()
            .filter(|s| s.pinned.unwrap_or(false))
            .collect();
        pinned.sort_by_key(|s| std::cmp::Reverse(s.note));

        let pinned_total = pinned.len();
        let pinned_in_budget = pinned_budget.min(pinned_total);
        let pinned_withheld = pinned_total - pinned_in_budget;

        // Every note number anything depends on, collected in ONE pass. Asking
        // `direct_dependents` per candidate is a full scan per candidate, which
        // on a 10k-note log is the quadratic the measurement test would find.
        let depended_on: HashSet<u32> = self
            .all_notes()
            .filter_map(|s| s.dependencies.as_ref())
            .flat_map(|deps| deps.iter().map(|e| e.note()))
            .collect();

        let lag_index = pinned_budget.saturating_add(PIN_DECAY_LAG);
        let mut candidates: Vec<DecayCandidate> = Vec::new();
        let mut candidates_total = 0usize;
        for (index, note) in pinned.iter().enumerate() {
            if index < lag_index {
                continue;
            }
            if !is_decay_candidate(note, &depended_on) {
                continue;
            }
            candidates_total += 1;
            if candidates.len() < MAINTENANCE_CANDIDATE_CAP {
                candidates.push(DecayCandidate {
                    note: note.note,
                    purpose: note.purpose.clone(),
                    outcome_excerpt: truncate_excerpt(&note.outcome, 80),
                    pinned_behind: index,
                });
            }
        }

        let pressure = if pinned_withheld == 0 {
            PinPressure::None
        } else if candidates_total == 0 {
            PinPressure::Building
        } else {
            PinPressure::Saturated
        };

        MaintenanceReport {
            pinned_total,
            pinned_in_budget,
            pinned_withheld,
            rollup_ceiling: pinned_budget + tail_budget,
            pressure,
            decay_candidates: candidates,
            decay_candidates_total: candidates_total,
            unpin_with: "decision_pin(note, pinned: false)",
        }
    }
}

/// Conditions 3 and 4 of the decay rule — the never-touched list.
///
/// Split out and taking only what it reads, so the rule can be stated and tested
/// without an engine around it. Every condition here is a reason to KEEP the
/// pin; the caller has already established that it is outside the budget.
fn is_decay_candidate(note: &Note, depended_on: &HashSet<u32>) -> bool {
    // Still cited: something in the retained window builds on it.
    if depended_on.contains(&note.note) {
        return false;
    }
    // Either side of a revision. A correction whose newer form is no longer
    // visible is worse than a saturated budget: the reader sees the claim that
    // was withdrawn and nothing that withdrew it.
    if note.revises_note.is_some() || note.revised_by.is_some() {
        return false;
    }
    true
}
