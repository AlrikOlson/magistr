//! Workflow messages update retained owner observations, never the conversation replay.

#[cfg(test)]
#[path = "session_workflow.tests.rs"]
mod tests;

use magistr_cockpit::engine::EngineEvent;
use magistr_engine::{Delivery, NativeEvent, NativeSnapshot, ReadError, ReadErrorKind, ReadState};

/// Returns whether the session must request a fresh owner read.
pub(super) fn observe(snapshot: &mut NativeSnapshot, event: EngineEvent) -> bool {
    match event {
        EngineEvent::Seed(read) => {
            *snapshot = read;
            false
        }
        EngineEvent::Event {
            delivery: Delivery::Duplicate,
            ..
        } => false,
        EngineEvent::Event { event, delivery } => {
            if delivery.needs_snapshot() {
                invalidate_all(snapshot);
            } else {
                match event {
                    NativeEvent::Roadmap(_) => invalidate(&mut snapshot.roadmap),
                    NativeEvent::Ship(_) => invalidate(&mut snapshot.ship),
                    NativeEvent::Signal(_) => invalidate(&mut snapshot.signals),
                    NativeEvent::Think(_) => invalidate(&mut snapshot.think),
                    NativeEvent::Magistr(_) => return false,
                }
            }
            true
        }
        EngineEvent::Refresh => {
            invalidate_all(snapshot);
            true
        }
        EngineEvent::Feed(_) => {
            invalidate_all(snapshot);
            true
        }
        EngineEvent::Pinned { .. } | EngineEvent::Checkpoint { .. } => {
            invalidate(&mut snapshot.think);
            true
        }
        EngineEvent::PortFailed { .. } => false,
    }
}

fn invalidate<T>(state: &mut ReadState<T>) {
    if matches!(state, ReadState::Current(_)) {
        state.record(Err(ReadError::new(
            ReadErrorKind::Changed,
            "Work records changed. Reading them again.",
        )));
    }
}

fn invalidate_all(snapshot: &mut NativeSnapshot) {
    invalidate(&mut snapshot.roadmap);
    invalidate(&mut snapshot.ship);
    invalidate(&mut snapshot.signals);
    invalidate(&mut snapshot.think);
}
