use super::{SignalError, SignalUseCases};
use crate::roadmap::domain::ChunkStatus;
use crate::roadmap::engine::RoadmapMutationError;
#[cfg(test)]
use crate::signal::domain::Signal;
use crate::signal::domain::{SignalKind, SignalStatus};

pub use magistr_workflow_contracts::signal::SignalPromotion;

impl SignalUseCases {
    /// Retry safely using the same work item and reciprocal links.
    pub fn promote(&self, id: &str) -> Result<SignalPromotion, SignalError> {
        let roadmap = self.roadmap.as_ref().ok_or(SignalError::NotWired)?;
        let _operation = self.promotion.lock().map_err(|_| SignalError::Poisoned)?;
        let signal = self.get(id)?;
        if !matches!(
            signal.status,
            SignalStatus::Researched | SignalStatus::Surfaced | SignalStatus::Promoted
        ) {
            return Err(SignalError::InvalidState(
                "signal must be researched or surfaced before it can be promoted".into(),
            ));
        }
        let chunk_id = signal
            .cross_refs
            .iter()
            .find_map(|reference| reference.strip_prefix("chunk:"))
            .map(str::to_owned)
            .unwrap_or_else(|| chunk_id_for(id));
        let chunk = roadmap
            .lock()
            .map_err(|_| SignalError::Poisoned)?
            .mutate(|edit| {
                let created = !edit
                    .roadmap()
                    .chunks
                    .iter()
                    .any(|chunk| chunk.id == chunk_id);
                if created {
                    edit.add_chunk(
                        chunk_id.clone(),
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
                    )?;
                }
                let chunk = edit.link_chunk(&chunk_id, &format!("signal:{id}"))?.clone();
                Ok((created, chunk))
            })
            .map_err(|error| match error {
                RoadmapMutationError::Rejected(message) => SignalError::InvalidState(message),
                RoadmapMutationError::Store(error) => SignalError::Store(error),
            })?;
        let created = chunk.value.0;
        let chunk = chunk.map(|(_, chunk)| chunk);
        let signal_result = self
            .engine
            .lock()
            .map_err(|_| SignalError::Poisoned)
            .and_then(|mut engine| {
                engine
                    .mutate(|edit| {
                        let current = edit.get(id)?;
                        if current
                            .cross_refs
                            .iter()
                            .filter_map(|reference| reference.strip_prefix("chunk:"))
                            .any(|linked| linked != chunk_id)
                        {
                            return Err("The signal is already linked to another work item.".into());
                        }
                        if current.status != SignalStatus::Promoted {
                            edit.promote(id)?;
                        }
                        edit.link(id, &format!("chunk:{chunk_id}")).cloned()
                    })
                    .map_err(SignalError::invalid_state)
            });
        let signal = signal_result.map_err(|cause| SignalError::PartialPromotion {
            chunk_id: chunk_id.clone(),
            committed_source: chunk.source,
            cause: Box::new(cause),
        })?;
        Ok(SignalPromotion {
            signal_id: id.to_owned(),
            chunk_id,
            created,
            signal,
            chunk,
        })
    }
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
