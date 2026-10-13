//! Bounded resident-session observations and subscription cursors change together.
//! The application retains the projection; hiding or disconnecting a reader does
//! not close the provider. Gaps require a retained-state reseed, never invented deltas.
use magistr_application::sessions::AgentEvent;
use magistr_cockpit::cockpit::{Msg, ProjectSeed};
use magistr_cockpit_runtime::channel::ObservationSink;
use magistr_domain::identity::SessionAddress;
use std::{
    collections::BTreeMap,
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicBool, Ordering},
};
use tokio::sync::{Notify, watch};

const MAX_MESSAGES: usize = 512;
const MAX_BYTES: usize = 1024 * 1024;
const MAX_REQUESTS: usize = 64;

#[derive(Clone, Debug, PartialEq)]
pub struct SessionSnapshot {
    pub address: SessionAddress,
    pub sequence: u64,
    pub seed: ProjectSeed,
    pub messages: Vec<(u64, Msg)>,
    pub pending: BTreeMap<u64, Msg>,
    pub dropped: u64,
    pub closed: bool,
    bytes: usize,
    retained_from: u64,
    answered: std::collections::BTreeSet<u64>,
    seed_at: u64,
    usage: Option<Msg>,
}

impl SessionSnapshot {
    /// A complete bounded observation, with pending requests retained separately.
    #[must_use]
    pub fn replay(&self) -> Vec<Msg> {
        let mut messages = Vec::new();
        if self.dropped > 0 {
            messages.push(Msg::Agent(AgentEvent::Lagged {
                bytes: 0,
                items: usize::try_from(self.dropped).unwrap_or(usize::MAX),
            }));
        }
        messages.extend(
            self.messages
                .iter()
                .filter(|(at, msg)| {
                    !matches!(
                        msg,
                        Msg::Agent(AgentEvent::PermissionRequest { .. } | AgentEvent::Usage { .. })
                    ) && (!matches!(msg, Msg::Engine(_)) || *at > self.seed_at)
                })
                .map(|(_, msg)| msg.clone()),
        );
        messages.extend(self.usage.iter().cloned());
        messages.extend(self.pending.values().cloned());
        messages
    }

    /// None means the requested cursor fell outside retention: reseed from replay.
    #[must_use]
    pub fn since(&self, sequence: u64) -> Option<Vec<Msg>> {
        if sequence > self.sequence || sequence.saturating_add(1) < self.retained_from {
            return None;
        }
        Some(
            self.messages
                .iter()
                .filter(|(at, _)| *at > sequence)
                .map(|(_, msg)| msg.clone())
                .collect(),
        )
    }

    fn record(&mut self, message: Msg) {
        self.sequence = self.sequence.saturating_add(1);
        match &message {
            Msg::Agent(AgentEvent::PromptRejected { text, .. })
            | Msg::CommandFailed {
                cmd:
                    magistr_cockpit::cockpit::Cmd::Prompt(text)
                    | magistr_cockpit::cockpit::Cmd::RoadmapPrompt(text),
                ..
            } if self.seed.draft.as_ref().is_none_or(String::is_empty) => {
                self.seed.draft = Some(text.clone());
            }
            Msg::Agent(AgentEvent::Usage { .. }) => self.usage = Some(message.clone()),
            Msg::Agent(AgentEvent::PermissionRequest { id, .. }) => {
                self.answered.remove(id);
                self.pending.insert(*id, message.clone());
            }
            Msg::Agent(
                AgentEvent::PermissionLapsed { id, .. } | AgentEvent::PermissionCancelled { id },
            ) => {
                self.pending.remove(id);
                self.answered.remove(id);
            }
            Msg::CommandFailed {
                cmd: magistr_cockpit::cockpit::Cmd::Permission { id, .. },
                ..
            } => {
                self.answered.remove(id);
            }
            _ => {}
        }
        self.bytes = self.bytes.saturating_add(weight(&message));
        self.messages.push((self.sequence, message));
        while self.messages.len() > MAX_MESSAGES || self.bytes > MAX_BYTES {
            let (sequence, removed) = self.messages.remove(0);
            self.retained_from = sequence.saturating_add(1);
            self.bytes = self.bytes.saturating_sub(weight(&removed));
            self.dropped = self.dropped.saturating_add(1);
        }
    }
}

fn weight(message: &Msg) -> usize {
    match message {
        Msg::Agent(
            AgentEvent::Text(text) | AgentEvent::Thought(text) | AgentEvent::UserText(text),
        ) => text.len() + 64,
        Msg::Agent(AgentEvent::Terminal { bytes, .. }) => bytes.len() + 64,
        other => format!("{other:?}").len() + 64,
    }
}

#[derive(Debug)]
pub(super) struct SessionObservation {
    state: watch::Sender<SessionSnapshot>,
    closed: AtomicBool,
    closing: Notify,
    reader: std::sync::OnceLock<magistr_engine::NativeReader>,
    wake: std::sync::OnceLock<std::sync::Arc<Notify>>,
}

impl SessionObservation {
    pub(super) fn new(address: SessionAddress) -> Self {
        Self {
            state: watch::channel(SessionSnapshot {
                address,
                sequence: 0,
                seed: ProjectSeed::default(),
                messages: Vec::new(),
                pending: BTreeMap::new(),
                dropped: 0,
                closed: false,
                bytes: 0,
                retained_from: 1,
                answered: std::collections::BTreeSet::new(),
                seed_at: 0,
                usage: None,
            })
            .0,
            closed: AtomicBool::new(false),
            closing: Notify::new(),
            reader: std::sync::OnceLock::new(),
            wake: std::sync::OnceLock::new(),
        }
    }

    pub(super) fn seed(&self, seed: ProjectSeed) {
        self.state.send_modify(|state| state.seed = seed);
    }
    pub(super) fn draft(&self, text: String) {
        self.state
            .send_modify(|state| state.seed.draft = Some(text));
    }
    pub(super) fn snapshot(&self) -> SessionSnapshot {
        self.state.borrow().clone()
    }
    pub(super) fn wake_with(&self, wake: std::sync::Arc<Notify>) {
        let _ = self.wake.set(wake);
    }
    pub(super) fn set_reader(&self, reader: magistr_engine::NativeReader) {
        let _ = self.reader.set(reader);
    }
    pub(super) fn subscribe(self: &std::sync::Arc<Self>) -> SessionSubscription {
        SessionSubscription {
            receiver: self.state.subscribe(),
            observation: std::sync::Arc::clone(self),
        }
    }
    pub(super) fn is_answered(&self, id: u64) -> bool {
        self.state.borrow().answered.contains(&id)
    }
    pub(super) fn permission_answered(&self, id: u64) {
        self.state.send_modify(|state| {
            state.answered.insert(id);
        });
    }
    pub(super) fn stop(&self) {
        self.closed.store(true, Ordering::Release);
        self.state.send_modify(|state| {
            state.closed = true;
            state.sequence = state.sequence.saturating_add(1);
        });
        self.closing.notify_waiters();
        if let Some(wake) = self.wake.get() {
            wake.notify_one();
        }
    }
}

impl ObservationSink for SessionObservation {
    fn send(&self, message: Msg) -> bool {
        if self.is_closed() {
            return false;
        }
        let mut overloaded = false;
        self.state.send_modify(|state| {
            if state.closed { return; }
            if let Msg::Agent(AgentEvent::PermissionRequest { id, .. }) = &message {
                let bytes: usize = state.pending.values().map(weight).sum();
                if !state.pending.contains_key(id) && (state.pending.len() >= MAX_REQUESTS || bytes.saturating_add(weight(&message)) > MAX_BYTES) {
                    state.record(Msg::Agent(AgentEvent::Error("The agent sent too many requests and was stopped. Open a new session to continue.".into())));
                    overloaded = true;
                    return;
                }
            }
            state.record(message);
        });
        if overloaded {
            self.stop();
        }
        !overloaded
    }
    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }
    fn closed(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(async move {
            let notified = self.closing.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if !self.is_closed() {
                notified.await;
            }
        })
    }
}

/// One reader's ordered cursor. Slow readers coalesce into the latest bounded
/// snapshot; sequence and dropped counts expose whether a replay is required.
pub struct SessionSubscription {
    receiver: watch::Receiver<SessionSnapshot>,
    observation: std::sync::Arc<SessionObservation>,
}

impl SessionSubscription {
    /// Inspect owner closure without advancing this reader's observation cursor.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.observation.is_closed()
    }

    /// Re-read workflow state through its owning reader before a retained-state reseed.
    pub async fn refresh(&mut self) -> Result<SessionSnapshot, String> {
        if let Some(reader) = self.observation.reader.get().cloned() {
            let at = self.receiver.borrow().sequence;
            let native = tokio::task::spawn_blocking(move || reader.snapshot())
                .await
                .map_err(|error| error.to_string())?;
            self.observation.state.send_modify(|state| {
                state.seed.snapshot = native;
                state.seed_at = at;
            });
        }
        Ok(self.snapshot())
    }
    pub fn snapshot(&mut self) -> SessionSnapshot {
        self.receiver.borrow_and_update().clone()
    }
    pub async fn changed(&mut self) -> Option<SessionSnapshot> {
        self.receiver.changed().await.ok()?;
        Some(self.snapshot())
    }
}

#[cfg(test)]
#[path = "session_observation.tests.rs"]
mod tests;
