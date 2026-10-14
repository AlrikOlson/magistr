//! Publish only final committed records; compound edits have one event per work item.
use super::{RoadmapEngine, RoadmapFrame};
use std::collections::HashSet;

impl RoadmapEngine {
    pub(super) fn publish_committed(&self, frames: Vec<RoadmapFrame>) {
        let added: HashSet<String> = frames
            .iter()
            .filter_map(|frame| match frame {
                RoadmapFrame::ChunkAdded { chunk } => Some(chunk.id.clone()),
                _ => None,
            })
            .collect();
        let mut seen = HashSet::new();
        let mut committed = Vec::new();
        for frame in frames.into_iter().rev() {
            let old = match &frame {
                RoadmapFrame::ChunkAdded { chunk }
                | RoadmapFrame::ChunkChanged { chunk }
                | RoadmapFrame::ChunkCompleted { chunk }
                | RoadmapFrame::ChunkObsoleted { chunk } => chunk,
                RoadmapFrame::RefreshRecorded { .. } => {
                    committed.push(frame);
                    continue;
                }
            };
            if !seen.insert(old.id.clone()) {
                continue;
            }
            let Some(chunk) = self
                .roadmap
                .chunks
                .iter()
                .find(|chunk| chunk.id == old.id)
                .cloned()
            else {
                continue;
            };
            let frame = if added.contains(&chunk.id) {
                RoadmapFrame::ChunkAdded { chunk }
            } else {
                match frame {
                    RoadmapFrame::ChunkCompleted { .. } => RoadmapFrame::ChunkCompleted { chunk },
                    RoadmapFrame::ChunkObsoleted { .. } => RoadmapFrame::ChunkObsoleted { chunk },
                    _ => RoadmapFrame::ChunkChanged { chunk },
                }
            };
            committed.push(frame);
        }
        for frame in committed.into_iter().rev() {
            self.record_event(frame);
        }
    }
}
