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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SessionStatus {
    #[default]
    Ready,
    Working,
    Finished,
    Stopped,
    Declined,
    LimitReached,
    Failed,
}
impl SessionStatus {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ready => "Ready",
            Self::Working => "Working",
            Self::Finished => "Finished",
            Self::Stopped => "Stopped",
            Self::Declined => "Declined",
            Self::LimitReached => "Limit reached",
            Self::Failed => "Connection lost",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SessionSnapshot {
    pub address: SessionAddress,
    pub sequence: u64,
    pub seed: ProjectSeed,
    pub messages: Vec<(u64, Msg)>,
    pub pending: BTreeMap<u64, Msg>,
    pub dropped: u64,
    pub closed: bool,
    pub status: SessionStatus,
    pub model: Option<String>,
    pub conversation: Option<magistr_application::sessions::Conversation>,
    pub configuration: Vec<magistr_application::sessions::SessionConfig>,
    pub activity: Option<magistr_application::sessions::SessionActivity>,
    bytes: usize,
    retained_from: u64,
    answered: std::collections::BTreeSet<u64>,
    seed_at: u64,
    usage: Option<Msg>,
}

impl SessionSnapshot {
    /// Offered requests exclude accepted answers until a failed delivery restores them.
    pub fn pending_requests(&self) -> impl Iterator<Item = &Msg> {
        self.pending
            .iter()
            .filter(|(id, _)| {
                !self.closed
                    && !self
                        .activity
                        .as_ref()
                        .is_some_and(|activity| activity.cancellation_requested)
                    && !self.answered.contains(id)
            })
            .map(|(_, request)| request)
    }
    #[must_use]
    pub fn latest_usage(&self) -> Option<&Msg> {
        self.usage.as_ref()
    }
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
            Msg::InputAccepted(text) => {
                self.status = SessionStatus::Working;
                if self.seed.draft.as_ref() == Some(text) {
                    self.seed.draft = Some(String::new());
                }
            }
            Msg::PromptReplace(text) => self.seed.draft = Some(text.clone()),
            Msg::Agent(AgentEvent::Initialized { model, .. }) => {
                self.model = model.clone();
                self.status = SessionStatus::Ready;
            }
            Msg::Agent(AgentEvent::Conversation(conversation)) => {
                self.conversation = Some(conversation.clone())
            }
            Msg::Agent(AgentEvent::Configuration { options, model }) => {
                self.configuration.clone_from(options);
                self.model.clone_from(model);
            }
            Msg::Agent(AgentEvent::Activity(activity)) => self.activity = Some(activity.clone()),
            Msg::Agent(
                AgentEvent::Text(_) | AgentEvent::Thought(_) | AgentEvent::ToolCall { .. },
            ) => self.status = SessionStatus::Working,
            Msg::Agent(AgentEvent::TurnEnded(end)) => {
                self.status = match end {
                    magistr_application::sessions::TurnEnd::EndTurn => SessionStatus::Finished,
                    magistr_application::sessions::TurnEnd::Cancelled => SessionStatus::Stopped,
                    magistr_application::sessions::TurnEnd::Refusal => SessionStatus::Declined,
                    magistr_application::sessions::TurnEnd::MaxTokens
                    | magistr_application::sessions::TurnEnd::MaxTurnRequests => {
                        SessionStatus::LimitReached
                    }
                    magistr_application::sessions::TurnEnd::Other => SessionStatus::Finished,
                }
            }
            Msg::Agent(AgentEvent::Error(_)) => self.status = SessionStatus::Failed,
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
                status: SessionStatus::Ready,
                model: None,
                conversation: None,
                configuration: Vec::new(),
                activity: None,
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
        self.wake_catalog();
    }
    pub(super) fn draft(&self, text: String) {
        self.state.send_modify(|state| {
            if state.seed.draft.as_ref() != Some(&text) {
                state.record(Msg::PromptReplace(text));
            }
        });
    }
    pub(super) fn snapshot(&self) -> SessionSnapshot {
        self.state.borrow().clone()
    }
    pub(super) fn wake_with(&self, wake: std::sync::Arc<Notify>) {
        let _ = self.wake.set(wake);
    }
    fn wake_catalog(&self) {
        if let Some(wake) = self.wake.get() {
            wake.notify_one();
        }
    }
    pub(super) fn summary(&self) -> super::session_catalog::SessionSummary {
        super::session_catalog::SessionSummary::from_snapshot(&self.state.borrow())
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
            if state.answered.insert(id) {
                state.sequence = state.sequence.saturating_add(1);
            }
        });
        self.wake_catalog();
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
        let mut catalog_changed = false;
        self.state.send_modify(|state| {
            let streamed_content = matches!(&message, Msg::Agent(AgentEvent::Text(_) | AgentEvent::Thought(_) | AgentEvent::Terminal { .. }));
            let before = (!streamed_content || state.status != SessionStatus::Working)
                .then(|| super::session_catalog::SessionSummary::from_snapshot(state));
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
            catalog_changed = before.is_some_and(|before| before != super::session_catalog::SessionSummary::from_snapshot(state));
        });
        if catalog_changed {
            self.wake_catalog();
        }
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
