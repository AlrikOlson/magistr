use magistr_workflow::roadmap::domain::{Chunk, ChunkStatus};

use crate::roadmap::RoadmapProgress;

/// Project a magistr roadmap onto the engine's progress snapshot.
///
/// A chunk is a task; a workstream (`group`) is a phase. Obsoleted chunks
/// are history and count toward nothing — the roadmap that carries 73 of
/// them would otherwise read as one-third done before any work.
#[must_use]
pub fn progress_from_chunks(chunks: &[Chunk]) -> RoadmapProgress {
    let live = chunks.iter().filter(|c| c.status != ChunkStatus::Obsoleted);
    let mut total_tasks = 0u32;
    let mut checked_tasks = 0u32;
    // group → (live, done)
    let mut groups: std::collections::BTreeMap<&str, (u32, u32)> = Default::default();
    for chunk in live {
        total_tasks += 1;
        let done = chunk.status == ChunkStatus::Done;
        if done {
            checked_tasks += 1;
        }
        if let Some(group) = chunk.group.as_deref() {
            let entry = groups.entry(group).or_insert((0, 0));
            entry.0 += 1;
            if done {
                entry.1 += 1;
            }
        }
    }
    let total_phases = u32::try_from(groups.len()).unwrap_or(u32::MAX);
    let completed_phases = groups.values().filter(|(live, done)| live == done).count();
    RoadmapProgress {
        total_tasks,
        checked_tasks,
        total_phases,
        completed_phases: u32::try_from(completed_phases).unwrap_or(u32::MAX),
    }
}

#[cfg(test)]
#[path = "progress.tests.rs"]
mod tests;
