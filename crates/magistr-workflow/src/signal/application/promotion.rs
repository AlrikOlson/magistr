use std::sync::{Mutex, MutexGuard};

use crate::roadmap::domain::{Chunk, ChunkStatus};
use crate::roadmap::engine::RoadmapEngine;
use crate::signal::domain::{Signal, SignalKind, SignalStatus};

use super::{SignalError, SignalUseCases};

/// The native result of promotion, including the current signal on retries.
#[derive(Debug, Clone)]
pub struct SignalPromotion {
    pub signal_id: String,
    pub chunk_id: String,
    pub created: bool,
    pub signal: Signal,
    pub chunk: Option<Chunk>,
}

impl SignalUseCases {
    /// Promote once, retaining a durable reciprocal link on both records.
    /// Signal and roadmap locks are scoped separately; the shared operation
    /// lock serializes concurrent promotions through clones of this service.
    pub fn promote(&self, id: &str) -> Result<SignalPromotion, SignalError> {
        let roadmap = self.roadmap.as_ref().ok_or(SignalError::NotWired)?;
        let _operation = self.promotion.lock().map_err(|_| SignalError::Poisoned)?;
        let signal = self.get(id)?;
        if let Some(chunk_id) = signal
            .cross_refs
            .iter()
            .find_map(|reference| reference.strip_prefix("chunk:"))
            .map(str::to_owned)
        {
            let chunk = current_roadmap(roadmap)?
                .roadmap()
                .chunks
                .iter()
                .find(|chunk| chunk.id == chunk_id)
                .cloned();
            return Ok(SignalPromotion {
                signal_id: id.to_owned(),
                chunk_id,
                created: false,
                signal,
                chunk,
            });
        }

        let chunk_id = chunk_id_for(id);
        {
            let mut roadmap = current_roadmap(roadmap)?;
            let recovery = signal.status == SignalStatus::Promoted
                && roadmap.roadmap().chunks.iter().any(|chunk| {
                    chunk.id == chunk_id && chunk.cross_refs.contains(&format!("signal:{id}"))
                });
            if !recovery
                && !matches!(
                    signal.status,
                    SignalStatus::Researched | SignalStatus::Surfaced
                )
            {
                return Err(SignalError::InvalidState(
                    "signal must be researched or surfaced before it can be promoted".into(),
                ));
            }
            ensure_chunk(&mut roadmap, &signal, &chunk_id)?;
        }
        let signal = {
            let mut engine = self.lock()?;
            if engine.get(id).map_err(SignalError::NotFound)?.status != SignalStatus::Promoted {
                engine.promote(id).map_err(SignalError::InvalidState)?;
            }
            engine
                .link(id, &format!("chunk:{chunk_id}"))
                .cloned()
                .map_err(SignalError::InvalidState)?
        };
        Ok(SignalPromotion {
            signal_id: id.to_owned(),
            chunk_id,
            created: true,
            signal,
            chunk: None,
        })
    }
}

/// The roadmap engine, observed before promotion reads or writes it: the
/// chunk another role promoted is seen, never created twice.
fn current_roadmap(
    roadmap: &Mutex<RoadmapEngine>,
) -> Result<MutexGuard<'_, RoadmapEngine>, SignalError> {
    let mut engine = roadmap.lock().map_err(|_| SignalError::Poisoned)?;
    engine.refresh_if_stale().map_err(SignalError::Store)?;
    Ok(engine)
}

fn ensure_chunk(roadmap: &mut RoadmapEngine, signal: &Signal, id: &str) -> Result<(), SignalError> {
    // Retry a partial promotion by keeping the existing chunk and reasserting
    // its back-reference, rather than creating another chunk or losing work.
    if !roadmap.roadmap().chunks.iter().any(|chunk| chunk.id == id) {
        roadmap
            .add_chunk(
                id.to_owned(),
                promoted_title(signal.kind, &signal.body),
                ChunkStatus::Backlog,
                0,
                format!(
                    "Promoted from signal {}. Original: {}",
                    signal.id, signal.body
                ),
                Vec::new(),
                Vec::new(),
                false,
            )
            .map_err(SignalError::InvalidState)?;
    }
    roadmap
        .link_chunk(id, &format!("signal:{}", signal.id))
        .map_err(SignalError::InvalidState)?;
    Ok(())
}

fn chunk_id_for(signal_id: &str) -> String {
    format!("signal-{signal_id}")
}

fn promoted_title(kind: SignalKind, body: &str) -> String {
    let kind = match kind {
        SignalKind::Question => "question",
        SignalKind::Idea => "idea",
        SignalKind::Concern => "concern",
        SignalKind::Bug => "bug",
        SignalKind::Feedback => "feedback",
    };
    let first_line = body.lines().next().unwrap_or("").trim();
    let snippet: String = first_line.chars().take(80).collect();
    if snippet.is_empty() {
        format!("[{kind}] (signal)")
    } else {
        format!("[{kind}] {snippet}")
    }
}

#[cfg(test)]
#[path = "promotion.tests.rs"]
mod tests;
