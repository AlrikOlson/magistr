//! Workflow messages update shared owner observations, never conversation replay.
#[cfg(test)]
#[path = "session_workflow.tests.rs"]
mod tests;

use magistr_cockpit::engine::EngineEvent;
use magistr_engine::{Delivery, NativeEvent, NativeSnapshot, ReadError, ReadErrorKind, ReadState};
use std::sync::Arc;

/// Returns whether the session must request a fresh owner read.
pub(super) fn observe(snapshot: &mut Arc<NativeSnapshot>, event: EngineEvent) -> bool {
    match event {
        EngineEvent::Seed(read) => {
            if snapshot.as_ref() != &read {
                *snapshot = Arc::new(read);
            }
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
                    NativeEvent::Roadmap(_) => {
                        if matches!(&snapshot.roadmap, ReadState::Current(_)) {
                            invalidate(&mut Arc::make_mut(snapshot).roadmap);
                        }
                    }
                    NativeEvent::Ship(_) => {
                        if matches!(&snapshot.ship, ReadState::Current(_)) {
                            invalidate(&mut Arc::make_mut(snapshot).ship);
                        }
                    }
                    NativeEvent::Signal(_) => {
                        if matches!(&snapshot.signals, ReadState::Current(_)) {
                            invalidate(&mut Arc::make_mut(snapshot).signals);
                        }
                    }
                    NativeEvent::Think(_) => invalidate_think(snapshot),
                    NativeEvent::Magistr(_) => return false,
                }
            }
            true
        }
        EngineEvent::Refresh | EngineEvent::Feed(_) => {
            invalidate_all(snapshot);
            true
        }
        EngineEvent::Pinned { .. } | EngineEvent::Checkpoint { .. } => {
            invalidate_think(snapshot);
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

fn invalidate_think(snapshot: &mut Arc<NativeSnapshot>) {
    if matches!(&snapshot.think, ReadState::Current(_)) {
        invalidate(&mut Arc::make_mut(snapshot).think);
    }
}

fn invalidate_all(snapshot: &mut Arc<NativeSnapshot>) {
    let current = matches!(&snapshot.roadmap, ReadState::Current(_))
        || matches!(&snapshot.ship, ReadState::Current(_))
        || matches!(&snapshot.signals, ReadState::Current(_))
        || matches!(&snapshot.think, ReadState::Current(_));
    if current {
        let snapshot = Arc::make_mut(snapshot);
        invalidate(&mut snapshot.roadmap);
        invalidate(&mut snapshot.ship);
        invalidate(&mut snapshot.signals);
        invalidate(&mut snapshot.think);
    }
}
