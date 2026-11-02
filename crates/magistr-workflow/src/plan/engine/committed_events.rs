//! Publish only final committed records; compound edits have one event per work item.
use super::{PlanEngine, PlanFrame};
use std::collections::HashSet;

impl PlanEngine {
    pub(super) fn publish_committed(&self, frames: Vec<PlanFrame>) {
        let added: HashSet<String> = frames
            .iter()
            .filter_map(|frame| match frame {
                PlanFrame::TaskAdded { task } => Some(task.id.clone()),
                _ => None,
            })
            .collect();
        let mut seen = HashSet::new();
        let mut committed = Vec::new();
        for frame in frames.into_iter().rev() {
            let old = match &frame {
                PlanFrame::TaskAdded { task }
                | PlanFrame::TaskChanged { task }
                | PlanFrame::TaskCompleted { task }
                | PlanFrame::TaskRetired { task } => task,
                PlanFrame::RefreshRecorded { .. } => {
                    committed.push(frame);
                    continue;
                }
            };
            if !seen.insert(old.id.clone()) {
                continue;
            }
            let Some(task) = self
                .plan
                .tasks
                .iter()
                .find(|task| task.id == old.id)
                .cloned()
            else {
                continue;
            };
            let frame = if added.contains(&task.id) {
                PlanFrame::TaskAdded { task }
            } else {
                match frame {
                    PlanFrame::TaskCompleted { .. } => PlanFrame::TaskCompleted { task },
                    PlanFrame::TaskRetired { .. } => PlanFrame::TaskRetired { task },
                    _ => PlanFrame::TaskChanged { task },
                }
            };
            committed.push(frame);
        }
        for frame in committed.into_iter().rev() {
            self.record_event(frame);
        }
    }
}
