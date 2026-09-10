use std::sync::{Arc, Mutex, MutexGuard};

use crate::roadmap::domain::{Chunk, ChunkStatus};
use crate::roadmap::engine::RoadmapEngine;
use crate::signal::engine::SignalEngine;

use super::{ChunkDraft, RoadmapError};

/// Admitted native roadmap operations shared by MCP and local clients.
#[derive(Clone)]
pub struct RoadmapUseCases {
    pub(super) engine: Arc<Mutex<RoadmapEngine>>,
    pub(super) signal: Option<Arc<Mutex<SignalEngine>>>,
    pub(super) workspace_root: Option<std::path::PathBuf>,
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
        }
    }

    pub fn with_signal(mut self, signal: Arc<Mutex<SignalEngine>>) -> Self {
        self.signal = Some(signal);
        self
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

    pub fn complete_chunk(&self, id: &str, ship_ref: Option<&str>) -> Result<Chunk, RoadmapError> {
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
