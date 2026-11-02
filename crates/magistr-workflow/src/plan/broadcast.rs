//! Best-effort, fire-and-forget broadcast of plan mutations.
//!
//! Wire format: one newline-delimited JSON object per mutation with a
//! `family: "plan"` discriminator flattened on top of the typed
//! [`PlanFrame`] payload. The socket + fan-out tasks live in
//! [`crate::infra::broadcast`]; this is the typed view the plan engine
//! emits through, mirroring `crate::run::broadcast`.

use tracing::warn;

use crate::infra::{Broadcaster as EngineBroadcaster, Family};

pub use magistr_workflow_contracts::plan::PlanFrame;

/// Cheaply cloneable handle wrapping the shared [`EngineBroadcaster`], tagging
/// every emitted frame with `family: "plan"`.
#[derive(Clone)]
pub struct Broadcaster {
    inner: EngineBroadcaster,
}

impl Broadcaster {
    /// Wrap an existing engine broadcaster (already bound to the shared socket)
    /// so the plan family can emit through it without binding a second
    /// listener. Wired by the host's composition root.
    pub fn from_engine(inner: EngineBroadcaster) -> Self {
        Self { inner }
    }

    pub fn emit(&self, frame: &PlanFrame) {
        if let Err(e) = self.inner.emit(Family::Plan, frame) {
            warn!(
                target: "magistr_workflow::plan::broadcast",
                "dropping broadcast frame: {e}",
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::domain::{Task, TaskStatus};

    fn task() -> Task {
        Task {
            tier: None,
            id: "a".into(),
            title: "A".into(),
            name: crate::plan::name::derive("a"),
            status: TaskStatus::Done,
            priority: 1,
            description: String::new(),
            content: None,
            feature: None,
            notes: String::new(),
            acceptance: vec![],
            deps: vec![],
            refs: vec![],
            shared: true,
            reprioritize: None,
            status_proposal: None,
            title_proposal: None,
            retired_reason: None,
            waiting_on: None,
            project_id: None,
            created_at: "t".into(),
            updated_at: "t".into(),
        }
    }

    #[test]
    fn frame_serializes_with_type_tag() {
        let f = PlanFrame::TaskAdded { task: task() };
        let v = serde_json::to_value(&f).unwrap();
        assert_eq!(v["type"], "task_added");
        assert_eq!(v["task"]["id"], "a");
    }

    #[test]
    fn record_meta_marks_terminal_events() {
        assert_eq!(
            PlanFrame::TaskAdded { task: task() }.record_meta(),
            ("task", false)
        );
        assert_eq!(
            PlanFrame::TaskCompleted { task: task() }.record_meta(),
            ("task", true)
        );
        assert_eq!(
            PlanFrame::TaskRetired { task: task() }.record_meta(),
            ("task", true)
        );
        assert_eq!(
            PlanFrame::RefreshRecorded {
                summary: "s".into(),
                notes: vec![]
            }
            .record_meta(),
            ("refresh", false)
        );
    }
}
