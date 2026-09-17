//! Best-effort, fire-and-forget broadcast of signal mutations.
//!
//! Wire format: one newline-delimited JSON object per mutation with a
//! `family: "signal"` discriminator flattened on top of the typed
//! [`SignalFrame`] payload. The socket + fan-out tasks live in
//! [`crate::infra::broadcast`]; this is the typed view the signal engine
//! emits through, mirroring `crate::roadmap::broadcast`.

use tracing::warn;

use crate::infra::{Broadcaster as EngineBroadcaster, Family};

pub use magistr_workflow_contracts::signal::SignalFrame;

/// Cheaply cloneable handle wrapping the shared [`EngineBroadcaster`], tagging
/// every emitted frame with `family: "signal"`.
#[derive(Clone)]
pub struct Broadcaster {
    inner: EngineBroadcaster,
}

impl Broadcaster {
    /// Wrap an existing engine broadcaster (already bound to the shared socket)
    /// so the signal family can emit through it without binding a second
    /// listener. Wired by the host's composition root.
    pub fn from_engine(inner: EngineBroadcaster) -> Self {
        Self { inner }
    }

    pub fn emit(&self, frame: &SignalFrame) {
        if let Err(e) = self.inner.emit(Family::Signal, frame) {
            warn!(
                target: "magistr_workflow::signal::broadcast",
                "dropping broadcast frame: {e}",
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signal::domain::{Signal, SignalKind, SignalStatus};

    fn signal() -> Signal {
        Signal {
            id: "abc".into(),
            kind: SignalKind::Bug,
            from: "dana".into(),
            body: "it crashes".into(),
            content: None,
            created: "t".into(),
            status: SignalStatus::New,
            enrichment: vec![],
            cross_refs: vec![],
            surfaced_at: None,
            snooze_until: None,
            project_id: None,
        }
    }

    #[test]
    fn frame_serializes_with_type_tag() {
        let f = SignalFrame::SignalCaptured { signal: signal() };
        let v = serde_json::to_value(&f).unwrap();
        assert_eq!(v["type"], "signal_captured");
        assert_eq!(v["signal"]["id"], "abc");
    }

    #[test]
    fn changed_and_dismissed_tag_distinctly() {
        let c = serde_json::to_value(SignalFrame::SignalChanged { signal: signal() }).unwrap();
        assert_eq!(c["type"], "signal_changed");
        let d = serde_json::to_value(SignalFrame::SignalDismissed { signal: signal() }).unwrap();
        assert_eq!(d["type"], "signal_dismissed");
    }
}
