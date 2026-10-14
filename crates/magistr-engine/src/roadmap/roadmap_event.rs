use serde::{Deserialize, Serialize};

use super::roadmap_progress::RoadmapProgress;
use crate::native::NativeRoadmapSnapshot;

/// Events emitted by the roadmap manager.
///
/// These domain events notify consumers about roadmap state changes.
/// Infrastructure adapters translate concrete mutations (YAML writes,
/// task toggles) into these domain events.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RoadmapEvent {
    /// A task's checked state was toggled.
    TaskToggled {
        /// Phase identifier (e.g., "E1").
        phase_id: String,
        /// Task description text.
        task_text: String,
        /// New checked state.
        checked: bool,
    },

    /// A new task was added to the roadmap.
    TaskAdded {
        /// Phase identifier.
        phase_id: String,
        /// Task description text.
        task_text: String,
    },

    /// A task was removed from the roadmap.
    TaskRemoved {
        /// Phase identifier.
        phase_id: String,
        /// Task description text.
        task_text: String,
    },

    /// A phase reached 100% completion.
    PhaseCompleted {
        /// Phase identifier.
        phase_id: String,
        /// Phase name.
        phase_name: String,
    },

    /// Progress was updated (bulk refresh from disk reload, etc.).
    ProgressUpdated {
        /// Previous progress snapshot.
        previous: RoadmapProgress,
        /// New progress snapshot.
        current: RoadmapProgress,
    },

    /// The native think-and-ship roadmap was re-read (a chunk changed, a
    /// broadcast frame arrived, or a consumer asked for a refresh).
    NativeSnapshot {
        /// Counts, next ready chunk, current workstreams, history and progress.
        snapshot: Box<NativeRoadmapSnapshot>,
    },
}
