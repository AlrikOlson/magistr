//! One roadmap chunk as the Roadmap tab holds it.

use super::proposal_kind::ProposalKind;

/// A chunk parsed leniently off a store seed or a `chunk_*` broadcast
/// frame's payload: identity and status always, the detail fields as the
/// payload carries them, and every open proposal with its suggestion.
/// Only the id is required — the engines' wire shape is theirs to grow.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoadmapChunk {
    /// The chunk's stable id.
    pub id: String,
    /// The short node label (falls back to the id).
    pub name: String,
    /// The full claim.
    pub title: String,
    /// The status wire name (`pending`, `in_progress`, …).
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
    /// The chunk under a payload, when it carries an id.
    #[must_use]
    pub fn from_payload(chunk: &serde_json::Value) -> Option<Self> {
        let id = chunk["id"].as_str()?.to_owned();
        let text = |key: &str| chunk[key].as_str().unwrap_or_default().to_owned();
        let list = |key: &str| -> Vec<String> {
            chunk[key]
                .as_array()
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default()
        };
        let name = match chunk["name"].as_str() {
            Some(name) if !name.is_empty() => name.to_owned(),
            _ => id.clone(),
        };
        let proposals = ProposalKind::ALL
            .into_iter()
            .filter_map(|kind| {
                let open = chunk.get(kind.payload_key())?.as_object()?;
                let suggestion = match open.get(kind.suggestion_key())? {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Number(n) => n.to_string(),
                    _ => return None,
                };
                let reason = open
                    .get("reason")
                    .and_then(|r| r.as_str())
                    .unwrap_or_default()
                    .to_owned();
                Some((kind, suggestion, reason))
            })
            .collect();
        Some(Self {
            name,
            title: text("title"),
            status: text("status"),
            priority: u32::try_from(chunk["priority"].as_u64().unwrap_or(0)).unwrap_or(u32::MAX),
            group: chunk["group"].as_str().map(str::to_owned),
            deps: list("deps"),
            description: text("description"),
            acceptance: list("acceptance"),
            cross_refs: list("cross_refs"),
            proposals,
            updated_secs: chunk["updated_at"]
                .as_str()
                .and_then(|stamp| chrono::DateTime::parse_from_rfc3339(stamp).ok())
                .map(|stamp| stamp.timestamp()),
            id,
        })
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

#[cfg(test)]
#[path = "roadmap_chunk.tests.rs"]
mod tests;
