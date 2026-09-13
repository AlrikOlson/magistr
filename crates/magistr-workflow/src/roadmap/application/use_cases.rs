use std::sync::{Arc, Mutex, MutexGuard};

use crate::roadmap::domain::{Chunk, ChunkStatus};
use crate::roadmap::engine::RoadmapEngine;
use crate::ship::ShipUseCases;
use crate::signal::engine::SignalEngine;

use super::{ChunkDraft, RoadmapError};

/// Admitted native roadmap operations shared by MCP and local clients.
///
/// The ship family is the roadmap's completion authority, by construction:
/// a chunk completes only on that run's current evidence — every route —
/// MCP, an owner command, the cockpit, a loop — lands here, and none of
/// them can mark a chunk done another way. A ship family composed without
/// an evidence capability refuses every completion with the reason, so a
/// roadmap can never be composed over a permissive path.
#[derive(Clone)]
pub struct RoadmapUseCases {
    pub(super) engine: Arc<Mutex<RoadmapEngine>>,
    pub(super) signal: Option<Arc<Mutex<SignalEngine>>>,
    pub(super) workspace_root: Option<std::path::PathBuf>,
    pub(super) completion: ShipUseCases,
}

#[cfg(test)]
#[path = "use_cases.tests.rs"]
mod tests;

impl RoadmapUseCases {
    /// Compose the roadmap over `engine` with `completion` — the ship
    /// family whose receipts govern every route to `done`.
    pub fn new(engine: Arc<Mutex<RoadmapEngine>>, completion: ShipUseCases) -> Self {
        Self {
            engine,
            signal: None,
            workspace_root: None,
            completion,
        }
    }

    pub fn with_signal(mut self, signal: Arc<Mutex<SignalEngine>>) -> Self {
        self.signal = Some(signal);
        self
    }

    /// The refusal every route to `done` other than `complete_chunk`
    /// answers with: completion is decided on the run's receipts, never
    /// by naming the status.
    pub(super) fn refuse_direct_done(id: &str) -> RoadmapError {
        RoadmapError::InvalidState(format!(
            "chunk '{id}' cannot be set done directly: completion is governed by the run's verification receipts — use roadmap_complete_chunk"
        ))
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
            return Err(Self::refuse_direct_done(id));
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

    /// Mark `id` done with its proof-of-ship reference. The ship objective
    /// must be scoped to this chunk and its verification task — the `task:`
    /// reference when given, else the task holding the run's receipts —
    /// must carry current, passing, verified receipts for every required
    /// gate; the refusal names what is missing. The stored reference is the
    /// caller's, or the resolved task when none was given, so a completion
    /// always names its task.
    pub fn complete_chunk(&self, id: &str, ship_ref: Option<&str>) -> Result<Chunk, RoadmapError> {
        let scope = format!("chunk:{id}");
        let task = ship_ref
            .and_then(|reference| reference.trim().strip_prefix("task:"))
            .map(str::trim);
        let evidence = self
            .completion
            .verify_completion(&scope, task)
            .map_err(|refusal| RoadmapError::InvalidState(refusal.to_string()))?;
        let resolved = ship_ref
            .map(str::to_owned)
            .unwrap_or_else(|| format!("task:{}", evidence.task_id));
        self.lock()?
            .complete_chunk(id, Some(resolved.as_str()))
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
