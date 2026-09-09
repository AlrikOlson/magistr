//! A scripted engine port for the harness.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use magistr_cockpit::cockpit::ProposalKind;
use magistr_cockpit::engine::EnginePort;

/// An [`EnginePort`] made of a script: pins are accepted (or refused
/// for step numbers past a scripted ceiling) and remembered, a
/// checkpoint answers a scripted report, and the Roadmap tab's chunk,
/// proposal and signal actions act on scripted chunk and signal
/// payloads — the answer is the payload with the act applied, the way
/// the real engines answer. Clones share the log and the payloads, so a
/// test keeps one and boxes the other — the witness that a cockpit
/// click reached the engine and came back.
#[derive(Clone, Debug, Default)]
pub struct FakeEnginePort {
    pins: Arc<Mutex<Vec<(u32, bool)>>>,
    known_to: Option<u32>,
    report: serde_json::Value,
    chunks: Arc<Mutex<Vec<serde_json::Value>>>,
    signals: Arc<Mutex<Vec<serde_json::Value>>>,
    acts: Arc<Mutex<Vec<String>>>,
}

impl FakeEnginePort {
    /// A port that accepts every pin and answers `null` checkpoints.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The port refusing pins for steps past `number` — the real
    /// engine's unknown-step refusal.
    #[must_use]
    pub const fn knowing_steps_to(mut self, number: u32) -> Self {
        self.known_to = Some(number);
        self
    }

    /// The port answering a checkpoint with `report`.
    #[must_use]
    pub fn with_report(mut self, report: serde_json::Value) -> Self {
        self.report = report;
        self
    }

    /// The port knowing `chunk` (a chunk payload with at least an
    /// `id`); chunk and proposal acts answer with it, updated.
    #[must_use]
    pub fn with_chunk(self, chunk: serde_json::Value) -> Self {
        lock(&self.chunks).push(chunk);
        self
    }

    /// The port knowing `signal` (a signal payload with at least an
    /// `id`); signal acts answer with it, updated.
    #[must_use]
    pub fn with_signal(self, signal: serde_json::Value) -> Self {
        lock(&self.signals).push(signal);
        self
    }

    /// Every pin that reached the engine, in order.
    #[must_use]
    pub fn pins(&self) -> Vec<(u32, bool)> {
        lock(&self.pins).clone()
    }

    /// Every chunk, proposal and signal act that reached the engine, in
    /// order, as words (`start a`, `accept priority on a`, `promote s1`).
    #[must_use]
    pub fn acts(&self) -> Vec<String> {
        lock(&self.acts).clone()
    }

    fn update_chunk(
        &self,
        id: &str,
        act: String,
        update: impl FnOnce(&mut serde_json::Value),
    ) -> Result<serde_json::Value, String> {
        let mut chunks = lock(&self.chunks);
        let chunk = chunks
            .iter_mut()
            .find(|c| c["id"].as_str() == Some(id))
            .ok_or_else(|| format!("no chunk '{id}'"))?;
        update(chunk);
        lock(&self.acts).push(act);
        Ok(chunk.clone())
    }

    fn update_signal(
        &self,
        id: &str,
        act: String,
        update: impl FnOnce(&mut serde_json::Value),
    ) -> Result<serde_json::Value, String> {
        let mut signals = lock(&self.signals);
        let signal = signals
            .iter_mut()
            .find(|s| s["id"].as_str() == Some(id))
            .ok_or_else(|| format!("no signal '{id}'"))?;
        update(signal);
        lock(&self.acts).push(act);
        Ok(signal.clone())
    }
}

impl EnginePort for FakeEnginePort {
    fn pin(&mut self, step_number: u32, pinned: bool) -> Result<(), String> {
        if self.known_to.is_some_and(|last| step_number > last) {
            return Err(format!(
                "No step #{step_number} found in the current trace."
            ));
        }
        lock(&self.pins).push((step_number, pinned));
        Ok(())
    }

    fn checkpoint(&mut self) -> Result<serde_json::Value, String> {
        Ok(self.report.clone())
    }

    fn start_chunk(&mut self, id: &str) -> Result<serde_json::Value, String> {
        self.update_chunk(id, format!("start {id}"), |chunk| {
            chunk["status"] = "in_progress".into();
        })
    }

    fn complete_chunk(&mut self, id: &str) -> Result<serde_json::Value, String> {
        self.update_chunk(id, format!("complete {id}"), |chunk| {
            chunk["status"] = "done".into();
        })
    }

    fn resolve_proposal(
        &mut self,
        id: &str,
        kind: ProposalKind,
        accept: bool,
    ) -> Result<serde_json::Value, String> {
        let verb = if accept { "accept" } else { "reject" };
        self.update_chunk(id, format!("{verb} {} on {id}", kind.label()), |chunk| {
            let suggestion = chunk[kind.payload_key()][kind.suggestion_key()].clone();
            if accept && !suggestion.is_null() {
                let field = match kind {
                    ProposalKind::Reprioritize => "priority",
                    ProposalKind::Status => "status",
                    ProposalKind::Title => "title",
                };
                chunk[field] = suggestion;
            }
            if let Some(object) = chunk.as_object_mut() {
                object.remove(kind.payload_key());
            }
        })
    }

    fn clear_blocker(&mut self, id: &str) -> Result<serde_json::Value, String> {
        self.update_chunk(id, format!("clear the blocker on {id}"), |chunk| {
            if let Some(object) = chunk.as_object_mut() {
                object.remove("blocked_by");
            }
        })
    }

    fn promote_signal(&mut self, id: &str) -> Result<serde_json::Value, String> {
        self.update_signal(id, format!("promote {id}"), |signal| {
            signal["status"] = "promoted".into();
        })
    }

    fn ignore_signal(&mut self, id: &str) -> Result<serde_json::Value, String> {
        self.update_signal(id, format!("ignore {id}"), |signal| {
            signal["status"] = "dismissed".into();
        })
    }

    fn snooze_signal(&mut self, id: &str, minutes: u32) -> Result<serde_json::Value, String> {
        self.update_signal(id, format!("snooze {id} for {minutes}m"), |signal| {
            signal["snooze_until"] = "soon".into();
        })
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
#[path = "fake_engine_port.tests.rs"]
mod tests;
