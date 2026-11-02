//! Best-effort, fire-and-forget broadcast of request mutations.
//!
//! Wire format: one newline-delimited JSON object per mutation with a
//! `family: "request"` discriminator flattened on top of the typed
//! [`RequestFrame`] payload. The socket + fan-out tasks live in
//! [`crate::infra::broadcast`]; this is the typed view the request engine
//! emits through, mirroring `crate::plan::broadcast`.

use tracing::warn;

use crate::infra::{Broadcaster as EngineBroadcaster, Family};

pub use magistr_workflow_contracts::request::RequestFrame;

/// Cheaply cloneable handle wrapping the shared [`EngineBroadcaster`], tagging
/// every emitted frame with `family: "request"`.
#[derive(Clone)]
pub struct Broadcaster {
    inner: EngineBroadcaster,
}

impl Broadcaster {
    /// Wrap an existing engine broadcaster (already bound to the shared socket)
    /// so the request family can emit through it without binding a second
    /// listener. Wired by the host's composition root.
    pub fn from_engine(inner: EngineBroadcaster) -> Self {
        Self { inner }
    }

    pub fn emit(&self, frame: &RequestFrame) {
        if let Err(e) = self.inner.emit(Family::Request, frame) {
            warn!(
                target: "magistr_workflow::request::broadcast",
                "dropping broadcast frame: {e}",
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::request::domain::{Request, RequestKind, RequestStatus};

    fn request() -> Request {
        Request {
            id: "abc".into(),
            kind: RequestKind::Bug,
            from: "dana".into(),
            body: "it crashes".into(),
            content: None,
            created: "t".into(),
            status: RequestStatus::New,
            enrichment: vec![],
            refs: vec![],
            raised_at: None,
            snooze_until: None,
            project_id: None,
        }
    }

    #[test]
    fn frame_serializes_with_type_tag() {
        let f = RequestFrame::RequestCaptured { request: request() };
        let v = serde_json::to_value(&f).unwrap();
        assert_eq!(v["type"], "request_captured");
        assert_eq!(v["request"]["id"], "abc");
    }

    #[test]
    fn changed_and_dismissed_tag_distinctly() {
        let c = serde_json::to_value(RequestFrame::RequestChanged { request: request() }).unwrap();
        assert_eq!(c["type"], "request_changed");
        let d =
            serde_json::to_value(RequestFrame::RequestDismissed { request: request() }).unwrap();
        assert_eq!(d["type"], "request_dismissed");
    }
}
