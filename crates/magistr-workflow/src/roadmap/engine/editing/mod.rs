//! A candidate state and deferred events, with no persistence or publication access.
mod chunks;
mod imports;
mod proposals;
mod tracker;

use super::*;
use magistr_workflow_contracts::roadmap::{Readiness, WaitingDependency};

pub(crate) struct Edit<'a> {
    pub(super) roadmap: &'a mut Roadmap,
    pub(super) project_id: String,
    pub(super) inherit_opt_in: Option<String>,
    pub(super) events: Vec<RoadmapFrame>,
}

impl Edit<'_> {
    pub(crate) fn roadmap(&self) -> &Roadmap {
        self.roadmap
    }
    fn record_event(&mut self, frame: RoadmapFrame) {
        self.events.push(frame);
    }
    fn now() -> String {
        Utc::now().to_rfc3339()
    }

    fn index_of(&self, id: &str) -> Result<usize, String> {
        self.roadmap
            .chunks
            .iter()
            .position(|c| c.id == id)
            .ok_or_else(|| format!("chunk '{id}' not found"))
    }

    fn why_group_unfit(&self, name: &str, prospective_id: Option<&str>) -> Option<String> {
        let prefixes: Vec<String> = self
            .roadmap
            .chunks
            .iter()
            .map(|c| region::id_prefix(&c.id).to_string())
            .chain(prospective_id.map(|id| region::id_prefix(id).to_string()))
            .collect();
        region::why_unfit(name, prefixes.iter().map(String::as_str))
    }

    fn clean_group(group: Option<String>) -> Option<String> {
        group
            .map(|g| g.trim().to_string())
            .filter(|g| !g.is_empty())
    }

    pub fn workstreams(&self) -> WorkstreamCatalog {
        workstream_catalog(&self.roadmap.chunks)
    }

    fn normalize_ref(&self, raw: &str) -> Result<String, String> {
        CrossRef::from_wire(raw)
            .map(|r| r.to_wire())
            .map_err(|e| format!("invalid cross-ref '{raw}': {e}"))
    }

    fn push_cross_ref(&mut self, id: &str, raw: &str) -> Result<(), String> {
        let normalized = self.normalize_ref(raw)?;
        let idx = self.index_of(id)?;
        let chunk = &mut self.roadmap.chunks[idx];
        if !chunk.cross_refs.contains(&normalized) {
            chunk.cross_refs.push(normalized);
            chunk.updated_at = Self::now();
        }
        Ok(())
    }
}

impl Edit<'_> {
    pub(super) fn readiness<'a>(&self, chunk: &'a Chunk) -> Readiness<'a> {
        chunk.readiness(|id| {
            self.roadmap
                .chunks
                .iter()
                .find(|c| c.id == id)
                .map(|c| c.status)
        })
    }

    pub(super) fn waiting_reason(&self, deps: &[WaitingDependency<'_>]) -> String {
        format!(
            "Waiting for {}",
            deps.iter()
                .map(|dep| {
                    self.roadmap
                        .chunks
                        .iter()
                        .find(|c| c.id == dep.id)
                        .map_or("an unavailable work item", |c| c.name.as_str())
                })
                .collect::<Vec<_>>()
                .join(", ")
        )
    }

    pub(super) fn require_ready(&self, chunk: &Chunk) -> Result<(), String> {
        let reason = match self.readiness(chunk) {
            Readiness::Ready => return Ok(()),
            Readiness::Blocked(reason) => reason.to_owned(),
            Readiness::Waiting(deps) => self.waiting_reason(&deps),
            Readiness::Inactive(status) => match status {
                ChunkStatus::Backlog => "This work item is planned for later.",
                ChunkStatus::InProgress => "Work is already underway.",
                ChunkStatus::Blocked => {
                    "Resolve this work item's blocker and return it to the work queue."
                }
                ChunkStatus::Done => "This work item is already completed.",
                ChunkStatus::Obsoleted => "This work item has been replaced.",
                ChunkStatus::Pending => "This work item is waiting.",
            }
            .into(),
        };
        Err(format!("Cannot start {}: {reason}", chunk.name))
    }
}
