//! One roadmap chunk as the Roadmap tab holds it.

use magistr_engine::native::{Chunk, ChunkStatus};

use super::proposal_kind::ProposalKind;

/// A chunk as the Roadmap tab shows it, read off the store's own
/// [`Chunk`] record — from a snapshot or a `chunk_*` event: identity and
/// status always, the detail fields as the record carries them, and
/// every open proposal with its suggestion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoadmapChunk {
    /// The chunk's stable id.
    pub id: String,
    /// The short node label (falls back to the id).
    pub name: String,
    /// The full claim.
    pub title: String,
    /// The status, in the roadmap's own spelling (`pending`,
    /// `in_progress`, …) — the vocabulary the list's section headers and
    /// the sky's states are named in — with one word the store spells
    /// as two fields: `stuck`, a chunk in progress that carries a
    /// blocker (the star that breathes and wears the alert ring).
    pub status: String,
    /// Lower sorts earlier; 100/200/300/400 are the named bands.
    pub priority: u32,
    /// The workstream, when grouped.
    pub group: Option<String>,
    /// The chunks this one builds on.
    pub deps: Vec<String>,
    /// The plain-prose description.
    pub description: String,
    /// The acceptance bullets.
    pub acceptance: Vec<String>,
    /// The `think:`/`task:`/… cross-refs.
    pub cross_refs: Vec<String>,
    /// Every open proposal: its kind, the suggested value and the reason.
    pub proposals: Vec<(ProposalKind, String, String)>,
    /// When the record last changed, in seconds since the Unix epoch —
    /// for a shipped chunk, when it shipped — as the store's
    /// `updated_at` stamp carries it. `None` without an RFC 3339 stamp.
    pub updated_secs: Option<i64>,
}

impl RoadmapChunk {
    /// The row for the store's record.
    #[must_use]
    pub fn from_chunk(chunk: &Chunk) -> Self {
        let name = if chunk.name.is_empty() {
            chunk.id.clone()
        } else {
            chunk.name.clone()
        };
        Self {
            id: chunk.id.clone(),
            name,
            title: chunk.title.clone(),
            status: status_word(chunk).to_owned(),
            priority: chunk.priority,
            group: chunk.group.clone(),
            deps: chunk.deps.clone(),
            description: chunk.description.clone(),
            acceptance: chunk.acceptance.clone(),
            cross_refs: chunk.cross_refs.clone(),
            proposals: ProposalKind::ALL
                .into_iter()
                .filter_map(|kind| {
                    kind.on(chunk)
                        .map(|(suggestion, reason)| (kind, suggestion, reason))
                })
                .collect(),
            updated_secs: chrono::DateTime::parse_from_rfc3339(&chunk.updated_at)
                .ok()
                .map(|stamp| stamp.timestamp()),
        }
    }

    /// Whether the chunk still takes part in the plan.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.status != "obsoleted"
    }

    /// Whether this chunk is "you are here".
    #[must_use]
    pub fn is_in_progress(&self) -> bool {
        self.status == "in_progress"
    }

    /// Rows the chunk's detail card takes when it is open: the title,
    /// each non-empty line of the description, each acceptance bullet,
    /// and the deps and cross-refs lines when it has any. Counted from
    /// the same fields the view paints, so the Roadmap tab's scroll
    /// window and its rows never drift.
    #[must_use]
    pub fn detail_rows(&self) -> usize {
        1 + self.description.lines().filter(|l| !l.is_empty()).count()
            + self.acceptance.len()
            + usize::from(!self.deps.is_empty())
            + usize::from(!self.cross_refs.is_empty())
    }

    /// The band the priority falls in, as the roadmap spells it.
    #[must_use]
    pub const fn band(&self) -> &'static str {
        match self.priority {
            0..=100 => "critical",
            101..=200 => "high",
            201..=300 => "medium",
            _ => "low",
        }
    }
}

/// The state word for a record: its status as the roadmap spells it, or
/// `stuck` for work in progress that waits on a blocker.
fn status_word(chunk: &Chunk) -> &'static str {
    if chunk.status == ChunkStatus::InProgress && chunk.blocked_by.is_some() {
        return "stuck";
    }
    chunk.status.as_wire()
}

#[cfg(test)]
#[path = "roadmap_chunk.tests.rs"]
mod tests;
