use serde::{Deserialize, Serialize};

use super::plan_progress::PlanProgress;
use crate::native::NativePlanSnapshot;

/// Events emitted by the plan manager.
///
/// These domain events notify consumers about plan state changes.
/// Infrastructure adapters translate concrete mutations (YAML writes,
/// task toggles) into these domain events.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlanEvent {
    /// A task's checked state was toggled.
    TaskToggled {
        /// Phase identifier (e.g., "E1").
        phase_id: String,
        /// Task description text.
        task_text: String,
        /// New checked state.
        checked: bool,
    },

    /// A new task was added to the plan.
    TaskAdded {
        /// Phase identifier.
        phase_id: String,
        /// Task description text.
        task_text: String,
    },

    /// A task was removed from the plan.
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
        previous: PlanProgress,
        /// New progress snapshot.
        current: PlanProgress,
    },

    /// The native magistr plan was re-read (a task changed, a
    /// broadcast frame arrived, or a consumer asked for a refresh).
    NativeSnapshot {
        /// Counts, next ready task, current features, history and progress.
        snapshot: Box<NativePlanSnapshot>,
    },
}
