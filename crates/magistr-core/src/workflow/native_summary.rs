//! The think-and-ship roadmap as a [`SpecSummary`], so every consumer of
//! spec progress (the iteration loop, the workflow config, the prompt
//! context) reads chunks on a native project and never opens `spec.file`.

use magistr_engine::NativeReader;
use magistr_engine::native::{Chunk, ChunkStatus, ObjectiveStatus};

use super::spec::SpecSummary;
use crate::error::{MagistrError, Result};

/// Chunk progress as a spec summary: `total`/`checked` count chunks, and the
/// single unchecked item is the chunk `roadmap_next` would hand an agent —
/// so `all_done()` means "no ready chunk". Re-reads the roadmap first: the
/// agent's `magistr mcp-serve` writes it between turns.
#[must_use]
pub fn native_spec_summary(engines: &NativeReader) -> SpecSummary {
    engines.reload_roadmap();
    let progress = engines.roadmap_progress();
    let unchecked_items: Vec<String> = engines
        .roadmap_next()
        .map(|c| vec![format!("{} — {}", c.id, c.title)])
        .unwrap_or_default();
    let unchecked = unchecked_items.len();
    SpecSummary {
        total: progress.total_tasks as usize,
        checked: progress.checked_tasks as usize,
        unchecked,
        unchecked_items,
    }
}

/// Select work for an admitted loop without changing roadmap or objective state.
/// A saved target and an unfinished objective take precedence over ambiguity.
pub(crate) fn native_work_chunk(
    engines: &NativeReader,
    preferred: Option<&str>,
    track: Option<&str>,
) -> Result<Option<Chunk>> {
    engines.reload_roadmap();
    engines.reload_ship();
    let chunks = engines.roadmap_chunks();
    let in_track = |chunk: &&Chunk| track.is_none_or(|track| chunk.group.as_deref() == Some(track));
    let objective = engines.ship_objective().filter(|objective| {
        let settled_chunk = objective
            .scope
            .trim()
            .strip_prefix("chunk:")
            .and_then(|id| chunks.iter().find(|chunk| chunk.id == id.trim()))
            .is_some_and(|chunk| {
                matches!(chunk.status, ChunkStatus::Done | ChunkStatus::Obsoleted)
            });
        matches!(
            objective.status,
            ObjectiveStatus::Defined | ObjectiveStatus::Active
        ) && !settled_chunk
    });
    let saved = preferred.and_then(|id| chunks.iter().find(|chunk| chunk.id == id));
    if preferred.is_some() && saved.is_none() {
        return Err(MagistrError::Workflow(
            "The work saved for this run is no longer in the roadmap. Restore it before resuming this run."
                .into(),
        ));
    }
    if let Some(chunk) = saved
        && matches!(chunk.status, ChunkStatus::Pending | ChunkStatus::InProgress)
        && !in_track(&chunk)
    {
        return Err(MagistrError::Workflow(
            "The saved work belongs to another track. Resume that run before choosing a different track.".into(),
        ));
    }
    if let Some(chunk) = saved
        && matches!(chunk.status, ChunkStatus::Blocked | ChunkStatus::Backlog)
    {
        return Err(MagistrError::Workflow(format!(
            "Cannot resume “{}”: this work is {}.",
            work_name(chunk),
            if chunk.status == ChunkStatus::Blocked {
                "blocked"
            } else {
                "not scheduled"
            },
        )));
    }
    let saved = saved
        .filter(|chunk| matches!(chunk.status, ChunkStatus::Pending | ChunkStatus::InProgress));
    let objective_chunk = objective
        .as_ref()
        .and_then(|objective| objective.scope.trim().strip_prefix("chunk:"))
        .and_then(|id| chunks.iter().find(|chunk| chunk.id == id.trim()))
        .filter(|chunk| chunk.status == ChunkStatus::InProgress)
        .filter(in_track);
    let selected = match saved.or(objective_chunk) {
        Some(chunk) => Some(chunk.clone()),
        None => {
            let active: Vec<_> = chunks
                .iter()
                .filter(|chunk| chunk.status == ChunkStatus::InProgress)
                .filter(in_track)
                .collect();
            match active.as_slice() {
                [] => match track {
                    Some(track) => engines.roadmap_next_in_track(track),
                    None => engines.roadmap_next(),
                },
                [chunk] => Some((*chunk).clone()),
                _ => {
                    return Err(MagistrError::Workflow(format!(
                        "More than one work item is in progress: {}. Choose which work to resume.",
                        active
                            .iter()
                            .map(|chunk| format!("“{}”", work_name(chunk)))
                            .collect::<Vec<_>>()
                            .join(", "),
                    )));
                }
            }
        }
    };
    let Some(chunk) = selected else {
        return Ok(None);
    };
    ensure_available(&chunk, &chunks)?;
    Ok(Some(chunk))
}

fn ensure_available(chunk: &Chunk, chunks: &[Chunk]) -> Result<()> {
    if let Some(blocker) = &chunk.blocked_by {
        return Err(MagistrError::Workflow(format!(
            "Cannot resume “{}”: {}",
            work_name(chunk),
            blocker.reason,
        )));
    }
    for dependency in &chunk.deps {
        match chunks.iter().find(|candidate| candidate.id == *dependency) {
            Some(dependency) if dependency.status == ChunkStatus::Done => {}
            Some(dependency) => {
                return Err(MagistrError::Workflow(format!(
                    "Cannot resume “{}” until “{}” is complete.",
                    work_name(chunk),
                    work_name(dependency),
                )));
            }
            None => {
                return Err(MagistrError::Workflow(format!(
                    "Cannot resume “{}”: a required work item is missing from the roadmap.",
                    work_name(chunk),
                )));
            }
        }
    }
    Ok(())
}

fn work_name(chunk: &Chunk) -> &str {
    if !chunk.title.trim().is_empty() {
        chunk.title.trim()
    } else if !chunk.name.trim().is_empty() {
        chunk.name.trim()
    } else {
        "Untitled work item"
    }
}

/// Actionable progress for a loop. General reports retain `native_spec_summary`.
pub(crate) fn native_work_summary(
    engines: &NativeReader,
    preferred: Option<&str>,
    track: Option<&str>,
) -> Result<SpecSummary> {
    let chunk = native_work_chunk(engines, preferred, track)?;
    let progress = engines.roadmap_progress();
    let unchecked_items: Vec<_> = chunk
        .into_iter()
        .map(|chunk| {
            let suffix = if chunk.status == ChunkStatus::InProgress {
                " (in progress; resume this work)"
            } else {
                ""
            };
            format!("{} — {}{suffix}", chunk.id, chunk.title)
        })
        .collect();
    Ok(SpecSummary {
        total: progress.total_tasks as usize,
        checked: progress.checked_tasks as usize,
        unchecked: unchecked_items.len(),
        unchecked_items,
    })
}

#[cfg(test)]
#[path = "native_summary.tests.rs"]
mod tests;
