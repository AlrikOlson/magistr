//! The magistr roadmap as a [`SpecSummary`], so every consumer of
//! spec progress (the iteration loop, the workflow config, the prompt
//! context) reads chunks on a native project and never opens `spec.file`.

use magistr_engine::NativeReader;
use magistr_engine::native::{
    Chunk, ChunkStatus, FocusState, ObjectiveStatus, Readiness, WorkstreamResolution,
    assess_readiness, next_ready, progress_from_chunks, workstream_catalog,
};

use super::spec::SpecSummary;
use crate::error::{MagistrError, Result};

/// Progress counts all unfinished chunks; the item list selects the next
/// ready chunk for the prompt. Availability never means completion.
/// Re-reads the roadmap because the admitted writer may update it between turns.
pub fn native_spec_summary(engines: &NativeReader) -> Result<SpecSummary> {
    let chunks = engines.roadmap_chunks().map_err(MagistrError::Workflow)?;
    let progress = progress_from_chunks(&chunks);
    let unchecked_items: Vec<String> = next_ready(
        &chunks,
        |chunk| {
            chunk
                .readiness(|id| chunks.iter().find(|c| c.id == id).map(|c| c.status))
                .is_ready()
        },
        |chunk| chunk.priority,
    )
    .map(|c| vec![format!("{} — {}", c.id, c.title)])
    .unwrap_or_default();
    let unchecked = progress.total_tasks.saturating_sub(progress.checked_tasks) as usize;
    Ok(SpecSummary {
        total: progress.total_tasks as usize,
        checked: progress.checked_tasks as usize,
        unchecked,
        unchecked_items,
    })
}

/// Select work for an admitted loop without changing roadmap or objective state.
/// A saved target and an unfinished objective take precedence over ambiguity.
pub(crate) fn native_work_chunk(
    engines: &NativeReader,
    preferred: Option<&str>,
    track: Option<&str>,
) -> Result<Option<Chunk>> {
    let chunks = engines.roadmap_chunks().map_err(MagistrError::Workflow)?;
    select_work_chunk(engines, preferred, track, &chunks)
}

fn select_work_chunk(
    engines: &NativeReader,
    preferred: Option<&str>,
    track: Option<&str>,
    chunks: &[Chunk],
) -> Result<Option<Chunk>> {
    if let Some(track) = track {
        let catalog = workstream_catalog(chunks);
        let refusal = match catalog.focus_state(Some(track)) {
            FocusState::Current { .. } => None,
            FocusState::Closed { group } => Some(WorkstreamResolution::Closed {
                group,
                choices: catalog.current_names(),
            }),
            FocusState::Missing { .. } | FocusState::Unset => Some(WorkstreamResolution::Unknown {
                choices: catalog.current_names(),
            }),
        };
        if let Some(refusal) = refusal {
            return Err(MagistrError::Workflow(refusal.explain(track)));
        }
    }
    let in_track = |chunk: &&Chunk| track.is_none_or(|track| chunk.group.as_deref() == Some(track));
    let objective = engines
        .ship_objective()
        .map_err(MagistrError::Workflow)?
        .filter(|objective| {
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
                [] => next_ready(
                    chunks.iter().filter(in_track),
                    |chunk| {
                        chunk
                            .readiness(|id| chunks.iter().find(|c| c.id == id).map(|c| c.status))
                            .is_ready()
                    },
                    |chunk| chunk.priority,
                )
                .cloned(),
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
    ensure_available(&chunk, chunks)?;
    Ok(Some(chunk))
}

fn ensure_available(chunk: &Chunk, chunks: &[Chunk]) -> Result<()> {
    // The caller has selected pending or in-progress work. Resuming either
    // requires the same current blockers and dependencies as scheduling it.
    match assess_readiness(
        ChunkStatus::Pending,
        chunk
            .blocked_by
            .as_ref()
            .map(|blocker| blocker.reason.as_str()),
        &chunk.deps,
        |id| {
            chunks
                .iter()
                .find(|candidate| candidate.id == id)
                .map(|candidate| candidate.status)
        },
    ) {
        Readiness::Blocked(reason) => Err(MagistrError::Workflow(format!(
            "Cannot resume “{}”: {reason}",
            work_name(chunk),
        ))),
        Readiness::Waiting(dependencies) => {
            let dependency = dependencies.first().and_then(|dependency| {
                chunks
                    .iter()
                    .find(|candidate| candidate.id == dependency.id)
            });
            Err(MagistrError::Workflow(match dependency {
                Some(dependency) => format!(
                    "Cannot resume “{}” until “{}” is complete.",
                    work_name(chunk),
                    work_name(dependency),
                ),
                None => format!(
                    "Cannot resume “{}”: a required work item is missing from the roadmap.",
                    work_name(chunk),
                ),
            }))
        }
        Readiness::Ready | Readiness::Inactive(_) => Ok(()),
    }
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
    let chunks = engines.roadmap_chunks().map_err(MagistrError::Workflow)?;
    let chunk = select_work_chunk(engines, preferred, track, &chunks)?;
    let progress = progress_from_chunks(&chunks);
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
        unchecked: progress.total_tasks.saturating_sub(progress.checked_tasks) as usize,
        unchecked_items,
    })
}

#[cfg(test)]
#[path = "native_summary.tests.rs"]
mod tests;
