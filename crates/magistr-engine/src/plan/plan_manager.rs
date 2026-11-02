use serde::{Deserialize, Serialize};

use super::plan_event::PlanEvent;
use super::plan_progress::PlanProgress;

/// Domain service for plan progress tracking and event emission.
///
/// Maintains a progress snapshot and emits events on state changes.
/// Does not own the plan data — infrastructure adapters push
/// progress updates into the manager after mutations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanManager {
    /// Current progress snapshot.
    progress: PlanProgress,
}

impl PlanManager {
    /// Create a new plan manager with initial progress.
    #[must_use]
    pub fn new(progress: PlanProgress) -> Self {
        Self { progress }
    }

    /// Create a manager with no initial progress.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            progress: PlanProgress::default(),
        }
    }

    // ── Accessors ───────────────────────────────────────────────────────

    /// Current progress snapshot.
    #[must_use]
    pub fn progress(&self) -> &PlanProgress {
        &self.progress
    }

    /// Overall completion fraction (0.0–1.0).
    #[must_use]
    pub fn completion(&self) -> f64 {
        self.progress.completion()
    }

    /// Whether the spec is fully complete.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.progress.is_complete()
    }

    // ── Commands ────────────────────────────────────────────────────────

    /// Update progress from an external source (e.g., after reloading YAML).
    ///
    /// Returns a `ProgressUpdated` event if the progress actually changed.
    pub fn update_progress(&mut self, new: PlanProgress) -> Option<PlanEvent> {
        if self.progress == new {
            return None;
        }
        let previous = self.progress.clone();
        self.progress = new.clone();
        Some(PlanEvent::ProgressUpdated {
            previous,
            current: new,
        })
    }

    /// Record a task toggle. Updates checked count and returns events.
    ///
    /// The caller is responsible for performing the actual mutation in the
    /// plan store. This method updates the progress snapshot and emits
    /// the appropriate domain events.
    pub fn record_task_toggle(
        &mut self,
        phase_id: impl Into<String>,
        task_text: impl Into<String>,
        checked: bool,
        new_progress: PlanProgress,
    ) -> Vec<PlanEvent> {
        let phase_id = phase_id.into();
        let task_text = task_text.into();
        let mut events = vec![PlanEvent::TaskToggled {
            phase_id: phase_id.clone(),
            task_text,
            checked,
        }];

        let old_completed = self.progress.completed_phases;
        self.progress = new_progress;

        // Check if a phase just completed
        if self.progress.completed_phases > old_completed {
            events.push(PlanEvent::PhaseCompleted {
                phase_id,
                phase_name: String::new(), // Caller can fill this in
            });
        }

        events
    }

    /// Record a task addition. Updates total count.
    pub fn record_task_added(
        &mut self,
        phase_id: impl Into<String>,
        task_text: impl Into<String>,
        new_progress: PlanProgress,
    ) -> PlanEvent {
        self.progress = new_progress;
        PlanEvent::TaskAdded {
            phase_id: phase_id.into(),
            task_text: task_text.into(),
        }
    }

    /// Record a task removal. Updates total count.
    pub fn record_task_removed(
        &mut self,
        phase_id: impl Into<String>,
        task_text: impl Into<String>,
        new_progress: PlanProgress,
    ) -> PlanEvent {
        self.progress = new_progress;
        PlanEvent::TaskRemoved {
            phase_id: phase_id.into(),
            task_text: task_text.into(),
        }
    }
}
