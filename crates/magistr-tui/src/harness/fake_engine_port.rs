//! A scripted engine port for the harness.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use magistr_cockpit::cockpit::ProposalKind;
use magistr_cockpit::engine::EnginePort;
use magistr_engine::native::{Chunk, ChunkStatus, Signal, SignalStatus};

/// An [`EnginePort`] made of a script: pins are accepted (or refused
/// for step numbers past a scripted ceiling) and remembered, a
/// checkpoint answers a scripted report, and the Roadmap tab's chunk,
/// proposal and signal actions act on scripted chunk and signal
/// records — the answer is the record with the act applied, the way
/// the real engines answer. Clones share the log and the records, so a
/// test keeps one and boxes the other — the witness that a cockpit
/// click reached the engine and came back.
#[derive(Clone, Debug, Default)]
pub struct FakeEnginePort {
    pins: Arc<Mutex<Vec<(u32, bool)>>>,
    known_to: Option<u32>,
    report: serde_json::Value,
    chunks: Arc<Mutex<Vec<Chunk>>>,
    signals: Arc<Mutex<Vec<Signal>>>,
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

    /// The port knowing `chunk`; chunk and proposal acts answer with it,
    /// updated.
    #[must_use]
    pub fn with_chunk(self, chunk: Chunk) -> Self {
        lock(&self.chunks).push(chunk);
        self
    }

    /// The port knowing `signal`; signal acts answer with it, updated.
    #[must_use]
    pub fn with_signal(self, signal: Signal) -> Self {
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
        update: impl FnOnce(&mut Chunk),
    ) -> Result<Chunk, String> {
        let mut chunks = lock(&self.chunks);
        let chunk = chunks
            .iter_mut()
            .find(|c| c.id == id)
            .ok_or_else(|| format!("no chunk '{id}'"))?;
        update(chunk);
        lock(&self.acts).push(act);
        Ok(chunk.clone())
    }

    fn update_signal(
        &self,
        id: &str,
        act: String,
        update: impl FnOnce(&mut Signal),
    ) -> Result<Signal, String> {
        let mut signals = lock(&self.signals);
        let signal = signals
            .iter_mut()
            .find(|s| s.id == id)
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

    fn start_chunk(&mut self, id: &str) -> Result<Chunk, String> {
        self.update_chunk(id, format!("start {id}"), |chunk| {
            chunk.status = ChunkStatus::InProgress;
        })
    }

    fn complete_chunk(&mut self, id: &str) -> Result<Chunk, String> {
        self.update_chunk(id, format!("complete {id}"), |chunk| {
            chunk.status = ChunkStatus::Done;
        })
    }

    fn resolve_proposal(
        &mut self,
        id: &str,
        kind: ProposalKind,
        accept: bool,
    ) -> Result<Chunk, String> {
        let verb = if accept { "accept" } else { "reject" };
        self.update_chunk(
            id,
            format!("{verb} {} on {id}", kind.label()),
            |chunk| match kind {
                ProposalKind::Reprioritize => {
                    if let Some(proposal) = chunk.reprioritize.take()
                        && accept
                    {
                        chunk.priority = proposal.suggested_priority;
                    }
                }
                ProposalKind::Status => {
                    if let Some(proposal) = chunk.status_proposal.take()
                        && accept
                    {
                        chunk.status = proposal.suggested_status;
                    }
                }
                ProposalKind::Title => {
                    if let Some(proposal) = chunk.title_proposal.take()
                        && accept
                    {
                        chunk.title = proposal.suggested_title;
                    }
                }
            },
        )
    }

    fn clear_blocker(&mut self, id: &str) -> Result<Chunk, String> {
        self.update_chunk(id, format!("clear the blocker on {id}"), |chunk| {
            chunk.blocked_by = None;
        })
    }

    fn promote_signal(&mut self, id: &str) -> Result<Signal, String> {
        self.update_signal(id, format!("promote {id}"), |signal| {
            signal.status = SignalStatus::Promoted;
        })
    }

    fn ignore_signal(&mut self, id: &str) -> Result<Signal, String> {
        self.update_signal(id, format!("ignore {id}"), |signal| {
            signal.status = SignalStatus::Dismissed;
        })
    }

    fn snooze_signal(&mut self, id: &str, minutes: u32) -> Result<Signal, String> {
        self.update_signal(id, format!("snooze {id} for {minutes}m"), |signal| {
            signal.snooze_until = Some("soon".to_owned());
        })
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
#[path = "fake_engine_port.tests.rs"]
mod tests;
