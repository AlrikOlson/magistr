//! The cockpit's starting state, read off the engines before the feed speaks.

use magistr_engine::{NativeFrame, NativeReader};

/// What the cockpit shows before the first broadcast frame arrives: the
/// `ship_status` document, the roadmap's position and the pending
/// signals count, the Roadmap tab's chunks and signals, and the Trace
/// tab's reasoning steps and ship timeline — all read straight from the
/// in-process engines.
///
/// The entry points read one before the terminal is taken over, so an
/// iteration whose objective was recorded before the feed connected
/// shows its plan on the first frame; the loop driver reads a fresh one
/// at each iteration start (after `reload_ship`) and posts it as
/// [`EngineEvent::Seed`](super::EngineEvent::Seed).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StoreSeed {
    /// The full execution-state snapshot, as `ship_status` answers it.
    pub ship_status: serde_json::Value,
    /// The roadmap chunk currently `in_progress`, if one is started.
    pub chunk: Option<String>,
    /// Roadmap chunks done.
    pub done: u32,
    /// Roadmap chunks in total.
    pub total: u32,
    /// Signals captured and not yet promoted or dismissed.
    pub signals_pending: u32,
    /// Every chunk as the wire would carry it — the Roadmap tab's list.
    pub chunks: Vec<serde_json::Value>,
    /// Every signal likewise — the Roadmap tab's inbox.
    pub signals: Vec<serde_json::Value>,
    /// The persisted trace as the broadcast socket would have carried
    /// it: a `think`/`step_appended` frame per stored step, then the
    /// ship timeline's `action_recorded` and `check_recorded` frames in
    /// timestamp order. The Trace tab folds them through the one path it
    /// already has ([`Trace::seed`](crate::cockpit::Trace::seed)), so
    /// nothing here needs a second parser.
    pub trace: Vec<NativeFrame>,
}

impl StoreSeed {
    /// Read the cockpit's state off `engines` as the store holds it now.
    #[must_use]
    pub fn read(engines: &NativeReader) -> Self {
        let snapshot = engines.roadmap_snapshot();
        let counts = &engines.signal_status()["counts"];
        let count = |key: &str| u32::try_from(counts[key].as_u64().unwrap_or(0)).unwrap_or(0);
        Self {
            ship_status: engines.ship_status(),
            chunk: snapshot.in_progress.first().cloned(),
            done: snapshot.progress.checked_tasks,
            total: snapshot.progress.total_tasks,
            signals_pending: count("total")
                .saturating_sub(count("promoted"))
                .saturating_sub(count("dismissed")),
            chunks: engines
                .roadmap_chunks()
                .into_iter()
                .filter_map(|chunk| serde_json::to_value(chunk).ok())
                .collect(),
            signals: engines
                .signal_list()
                .into_iter()
                .filter_map(|signal| serde_json::to_value(signal).ok())
                .collect(),
            trace: trace_frames(engines),
        }
    }
}

/// The stored trace as frames: the reasoning steps in stored order, then
/// every action and check the execution trace holds, ordered by the
/// timestamp the engines stamped them with (one offset, so the strings
/// sort chronologically).
fn trace_frames(engines: &NativeReader) -> Vec<NativeFrame> {
    let mut frames: Vec<NativeFrame> = engines
        .think_steps_from_store()
        .iter()
        .filter_map(|step| serde_json::to_value(step).ok())
        .map(|step| {
            frame(
                "think",
                "step_appended",
                serde_json::json!({ "step": step }),
            )
        })
        .collect();

    let mut moments: Vec<(String, NativeFrame)> = Vec::new();
    for task in engines.ship_tasks() {
        for action in &task.actions {
            let Ok(action_value) = serde_json::to_value(action) else {
                continue;
            };
            moments.push((
                action.timestamp.clone(),
                frame(
                    "ship",
                    "action_recorded",
                    serde_json::json!({ "task_id": task.id, "action": action_value }),
                ),
            ));
        }
        for check in &task.checks {
            let Ok(check_value) = serde_json::to_value(check) else {
                continue;
            };
            moments.push((
                check.timestamp.clone(),
                frame(
                    "ship",
                    "check_recorded",
                    serde_json::json!({ "task_id": task.id, "check": check_value }),
                ),
            ));
        }
    }
    moments.sort_by(|(left, _), (right, _)| left.cmp(right));
    frames.extend(moments.into_iter().map(|(_, frame)| frame));
    frames
}

/// One frame with the two tags the socket flattens onto every payload.
fn frame(family: &str, kind: &str, mut payload: serde_json::Value) -> NativeFrame {
    if let Some(object) = payload.as_object_mut() {
        object.insert("family".to_owned(), serde_json::json!(family));
        object.insert("type".to_owned(), serde_json::json!(kind));
    }
    NativeFrame {
        family: family.to_owned(),
        kind: kind.to_owned(),
        payload,
    }
}

#[cfg(test)]
#[path = "seed.tests.rs"]
mod tests;
