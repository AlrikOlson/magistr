use std::sync::{Arc, Mutex, MutexGuard};

use crate::roadmap::domain::{Chunk, ChunkStatus};
use crate::roadmap::engine::RoadmapEngine;
use crate::ship::ShipUseCases;
use crate::signal::engine::SignalEngine;

use super::{ChunkDraft, RoadmapError};

/// Admitted native roadmap operations shared by MCP and local clients.
///
/// When the ship family is composed in as the completion authority, a
/// chunk completes only on that run's current evidence: every route —
/// MCP, an owner command, the cockpit, a loop — lands here, and none of
/// them can mark a chunk done another way.
#[derive(Clone)]
pub struct RoadmapUseCases {
    pub(super) engine: Arc<Mutex<RoadmapEngine>>,
    pub(super) signal: Option<Arc<Mutex<SignalEngine>>>,
    pub(super) workspace_root: Option<std::path::PathBuf>,
    pub(super) completion: Option<ShipUseCases>,
}

#[cfg(test)]
#[path = "use_cases.tests.rs"]
mod tests;

impl RoadmapUseCases {
    pub fn new(engine: Arc<Mutex<RoadmapEngine>>) -> Self {
        Self {
            engine,
            signal: None,
            workspace_root: None,
            completion: None,
        }
    }

    pub fn with_signal(mut self, signal: Arc<Mutex<SignalEngine>>) -> Self {
        self.signal = Some(signal);
        self
    }

    /// Make the ship family's receipts the authority over completion:
    /// `complete_chunk` consults [`ShipUseCases::verify_completion`] first,
    /// and the plain status routes refuse `done`.
    #[must_use]
    pub fn with_completion_evidence(mut self, ship: ShipUseCases) -> Self {
        self.completion = Some(ship);
        self
    }

    /// The composed completion authority, if any.
    #[must_use]
    pub fn completion_evidence(&self) -> Option<&ShipUseCases> {
        self.completion.as_ref()
    }

    /// The refusal every non-evidence route to `done` answers with while a
    /// completion authority is composed.
    pub(super) fn refuse_done_without_evidence(&self, id: &str) -> Result<(), RoadmapError> {
        if self.completion.is_some() {
            return Err(RoadmapError::InvalidState(format!(
                "chunk '{id}' cannot be set done directly: completion is governed by the run's verification receipts — use roadmap_complete_chunk"
            )));
        }
        Ok(())
    }

    pub fn engine(&self) -> Arc<Mutex<RoadmapEngine>> {
        Arc::clone(&self.engine)
    }

    pub fn with_workspace_root(
        mut self,
        root: impl Into<std::path::PathBuf>,
    ) -> std::io::Result<Self> {
        self.workspace_root = Some(std::path::absolute(root.into())?);
        Ok(self)
    }

    pub(super) fn lock(&self) -> Result<MutexGuard<'_, RoadmapEngine>, RoadmapError> {
        self.engine.lock().map_err(|_| RoadmapError::Poisoned)
    }

    pub fn add_chunk(&self, draft: ChunkDraft) -> Result<Chunk, RoadmapError> {
        if let Some(content) = &draft.content {
            content.validate().map_err(RoadmapError::InvalidArgs)?;
        }
        self.lock()?
            .add_chunk_with_content(
                draft.id,
                draft.title,
                draft.name,
                draft.status,
                draft.priority,
                draft.description,
                draft.acceptance,
                draft.deps,
                draft.shared,
                draft.content,
                draft.group,
            )
            .cloned()
            .map_err(RoadmapError::InvalidState)
    }

    pub fn set_status(&self, id: &str, status: ChunkStatus) -> Result<Chunk, RoadmapError> {
        if status == ChunkStatus::Done {
            self.refuse_done_without_evidence(id)?;
        }
        self.lock()?
            .set_status(id, status)
            .cloned()
            .map_err(RoadmapError::InvalidState)
    }

    pub fn start_chunk(&self, id: &str) -> Result<Chunk, RoadmapError> {
        self.lock()?
            .start_chunk(id)
            .cloned()
            .map_err(RoadmapError::InvalidState)
    }

    /// Mark `id` done with its proof-of-ship reference. With a completion
    /// authority composed, the ship objective must be scoped to this chunk
    /// and its verification task — the `task:` reference when given, else
    /// the task holding the run's receipts — must carry current, passing,
    /// verified receipts for every required gate; the refusal names what
    /// is missing. The stored reference is the caller's, or the resolved
    /// task when none was given, so a completion always names its task.
    pub fn complete_chunk(&self, id: &str, ship_ref: Option<&str>) -> Result<Chunk, RoadmapError> {
        let resolved;
        let ship_ref = match &self.completion {
            None => ship_ref,
            Some(ship) => {
                let scope = format!("chunk:{id}");
                let task = ship_ref
                    .and_then(|reference| reference.trim().strip_prefix("task:"))
                    .map(str::trim);
                let evidence = ship
                    .verify_completion(&scope, task)
                    .map_err(|refusal| RoadmapError::InvalidState(refusal.to_string()))?;
                resolved = ship_ref
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("task:{}", evidence.task_id));
                Some(resolved.as_str())
            }
        };
        self.lock()?
            .complete_chunk(id, ship_ref)
            .cloned()
            .map_err(RoadmapError::InvalidState)
    }

    pub fn obsolete_chunk(&self, id: &str, reason: String) -> Result<Chunk, RoadmapError> {
        self.lock()?
            .obsolete_chunk(id, reason)
            .cloned()
            .map_err(RoadmapError::InvalidState)
    }

    pub fn link_chunk(&self, id: &str, cross_ref: &str) -> Result<Chunk, RoadmapError> {
        self.lock()?
            .link_chunk(id, cross_ref)
            .cloned()
            .map_err(RoadmapError::InvalidArgs)
    }

    pub fn set_group(&self, id: &str, group: Option<String>) -> Result<Chunk, RoadmapError> {
        self.lock()?
            .set_group(id, group)
            .cloned()
            .map_err(RoadmapError::InvalidState)
    }

    pub fn record_refresh(
        &self,
        summary: String,
        think_steps: Vec<u32>,
    ) -> Result<usize, RoadmapError> {
        Ok(self.lock()?.record_refresh(summary, think_steps))
    }
}
