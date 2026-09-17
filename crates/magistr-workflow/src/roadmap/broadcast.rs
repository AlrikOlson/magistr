//! Best-effort, fire-and-forget broadcast of roadmap mutations.
//!
//! Wire format: one newline-delimited JSON object per mutation with a
//! `family: "roadmap"` discriminator flattened on top of the typed
//! [`RoadmapFrame`] payload. The socket + fan-out tasks live in
//! [`crate::infra::broadcast`]; this is the typed view the roadmap engine
//! emits through, mirroring `crate::ship::broadcast`.

use tracing::warn;

use crate::infra::{Broadcaster as EngineBroadcaster, Family};

pub use magistr_workflow_contracts::roadmap::RoadmapFrame;

/// Cheaply cloneable handle wrapping the shared [`EngineBroadcaster`], tagging
/// every emitted frame with `family: "roadmap"`.
#[derive(Clone)]
pub struct Broadcaster {
    inner: EngineBroadcaster,
}

impl Broadcaster {
    /// Wrap an existing engine broadcaster (already bound to the shared socket)
    /// so the roadmap family can emit through it without binding a second
    /// listener. Wired by the host's composition root.
    pub fn from_engine(inner: EngineBroadcaster) -> Self {
        Self { inner }
    }

    pub fn emit(&self, frame: &RoadmapFrame) {
        if let Err(e) = self.inner.emit(Family::Roadmap, frame) {
            warn!(
                target: "magistr_workflow::roadmap::broadcast",
                "dropping broadcast frame: {e}",
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roadmap::domain::{Chunk, ChunkStatus};

    fn chunk() -> Chunk {
        Chunk {
            tier: None,
            id: "a".into(),
            title: "A".into(),
            name: crate::roadmap::name::derive("a"),
            status: ChunkStatus::Done,
            priority: 1,
            description: String::new(),
            content: None,
            group: None,
            notes: String::new(),
            acceptance: vec![],
            deps: vec![],
            cross_refs: vec![],
            shared: true,
            reprioritize: None,
            status_proposal: None,
            title_proposal: None,
            obsoleted_reason: None,
            blocked_by: None,
            project_id: None,
            created_at: "t".into(),
            updated_at: "t".into(),
        }
    }

    #[test]
    fn frame_serializes_with_type_tag() {
        let f = RoadmapFrame::ChunkAdded { chunk: chunk() };
        let v = serde_json::to_value(&f).unwrap();
        assert_eq!(v["type"], "chunk_added");
        assert_eq!(v["chunk"]["id"], "a");
    }

    #[test]
    fn record_meta_marks_terminal_events() {
        assert_eq!(
            RoadmapFrame::ChunkAdded { chunk: chunk() }.record_meta(),
            ("chunk", false)
        );
        assert_eq!(
            RoadmapFrame::ChunkCompleted { chunk: chunk() }.record_meta(),
            ("chunk", true)
        );
        assert_eq!(
            RoadmapFrame::ChunkObsoleted { chunk: chunk() }.record_meta(),
            ("chunk", true)
        );
        assert_eq!(
            RoadmapFrame::RefreshRecorded {
                summary: "s".into(),
                think_steps: vec![]
            }
            .record_meta(),
            ("refresh", false)
        );
    }
}
