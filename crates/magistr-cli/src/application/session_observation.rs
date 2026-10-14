//! Bounded resident-session observations and subscription cursors change together.
//! The application retains the projection; hiding or disconnecting a reader does
//! not close the provider. Gaps require a retained-state reseed, never invented deltas.
use magistr_application::sessions::{AgentEvent, PromptInput, PromptReceipt};
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

/// Current submission outcome. It survives transcript pruning and is written
/// only after the shared input owner accepts or refuses the exact prompt.
#[derive(Clone, Debug, PartialEq)]
pub struct SubmissionAdmission {
    pub sequence: u64,
    pub input: PromptReceipt,
    pub accepted: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SessionSnapshot {
    pub address: SessionAddress,
    pub sequence: u64,
    pub seed: ProjectSeed,
    pub messages: Vec<super::RetainedObservation>,
    pub pending: BTreeMap<u64, Msg>,
    pub dropped: u64,
    pub closed: bool,
    pub status: SessionStatus,
    pub model: Option<String>,
    pub conversation: Option<magistr_application::sessions::Conversation>,
    pub configuration: Vec<magistr_application::sessions::SessionConfig>,
    pub attachments: Vec<super::StagedAttachment>,
    pub draft_revision: u64,
    pub draft_saved_revision: u64,
    pub draft_save_attempt: u64,
    pub draft_save_error: Option<String>,
    pub admission: Option<SubmissionAdmission>,
    pub activity: Option<magistr_application::sessions::SessionActivity>,
    bytes: usize,
    retained_from: u64,
    // An evicted active source keeps its kind/id so its tail cannot become a new reply.
    continuation: Option<(u64, super::retained_observation::StreamKind)>,
    answered: std::collections::BTreeSet<u64>,
    seed_at: u64,
    usage: Option<Msg>,
}

impl SessionSnapshot {
    #[must_use]
    pub fn draft_status(&self) -> super::DraftStatus {
        super::DraftStatus {
            revision: self.draft_revision,
            saved_revision: self.draft_saved_revision,
            save_attempt: self.draft_save_attempt,
            error: self.draft_save_error.clone(),
        }
    }
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
                .filter(|record| {
                    let msg = &record.message;
                    let at = record.sequence;
                    !matches!(
                        msg,
                        Msg::Agent(AgentEvent::PermissionRequest { .. } | AgentEvent::Usage { .. })
                    ) && (!matches!(msg, Msg::Engine(_)) || at > self.seed_at)
                })
                .map(|record| record.message.clone()),
        );
        messages.extend(self.usage.iter().cloned());
        messages.extend(self.pending.values().cloned());
        messages
    }

    /// None means the requested cursor fell outside retention: reseed from replay.
    #[must_use]
    pub fn since(&self, sequence: u64) -> Option<Vec<super::RetainedObservation>> {
        if sequence > self.sequence || sequence.saturating_add(1) < self.retained_from {
            return None;
        }
        Some(
            self.messages
                .iter()
                .filter(|record| record.sequence > sequence)
                .cloned()
                .collect(),
        )
    }

    fn record(&mut self, message: Msg) {
        let edits_draft = matches!(
            &message,
            Msg::InputAccepted(_)
                | Msg::PromptReplace(_)
                | Msg::Agent(AgentEvent::PromptRejected { .. })
                | Msg::CommandFailed {
                    cmd: magistr_cockpit::cockpit::Cmd::Prompt(_)
                        | magistr_cockpit::cockpit::Cmd::RoadmapPrompt(_),
                    ..
                }
        );
        let before = edits_draft.then(|| (self.seed.draft.clone(), self.attachments.clone()));
        self.sequence = self.sequence.saturating_add(1);
        let admission = match &message {
            Msg::InputAccepted(input) => Some((input.clone(), true)),
            Msg::Agent(AgentEvent::PromptRejected { input, .. })
            | Msg::CommandFailed {
                cmd:
                    magistr_cockpit::cockpit::Cmd::Prompt(input)
                    | magistr_cockpit::cockpit::Cmd::RoadmapPrompt(input),
                ..
            } => Some((input.receipt(), false)),
            _ => None,
        };
        if let Some((input, accepted)) = admission {
            self.admission = Some(SubmissionAdmission {
                sequence: self.sequence,
                input,
                accepted,
            });
        }
        match &message {
            Msg::InputAccepted(input) => {
                self.status = SessionStatus::Working;
                if self.seed.draft.as_ref() == Some(&input.text)
                    && super::session_attachments::matches_receipt(&self.attachments, input)
                {
                    self.seed.draft = Some(String::new());
                    self.attachments.clear();
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
            Msg::Agent(AgentEvent::PromptRejected { input, .. })
            | Msg::CommandFailed {
                cmd:
                    magistr_cockpit::cockpit::Cmd::Prompt(input)
                    | magistr_cockpit::cockpit::Cmd::RoadmapPrompt(input),
                ..
            } if self.seed.draft.as_ref().is_none_or(String::is_empty)
                && self.attachments.is_empty() =>
            {
                self.seed.draft = Some(input.text.clone());
                self.attachments = input
                    .attachments
                    .iter()
                    .cloned()
                    .map(super::StagedAttachment::sent)
                    .collect();
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
        if before.is_some_and(|(text, files)| text != self.seed.draft || files != self.attachments)
        {
            self.draft_revision = self.draft_revision.saturating_add(1);
        }
        self.retain(message);
    }

    fn retain(&mut self, message: Msg) {
        let streamed = super::retained_observation::StreamKind::of(&message);
        if matches!(
            &message,
            Msg::InputAccepted(_)
                | Msg::CommandFailed { .. }
                | Msg::Agent(
                    AgentEvent::TurnEnded(_)
                        | AgentEvent::Notice(_)
                        | AgentEvent::Error(_)
                        | AgentEvent::PromptRejected { .. }
                        | AgentEvent::PermissionLapsed { .. }
                        | AgentEvent::Lagged { .. }
                )
        ) {
            self.continuation = None;
        }
        let mut next = Some(super::RetainedObservation::new(self.sequence, message));
        if let Some(incoming) = next.as_ref() {
            if let Some(id) = incoming.tool_id() {
                if let Some(held) = self
                    .messages
                    .iter_mut()
                    .find(|held| held.tool_id() == Some(id))
                {
                    self.bytes = self.bytes.saturating_sub(weight(&held.message));
                    held.update_tool(
                        self.sequence,
                        next.take().expect("incoming record exists").message,
                    );
                    self.bytes = self.bytes.saturating_add(weight(&held.message));
                }
            } else if let Some(kind) = streamed
                && let Some((id, active_kind)) = self.continuation
                && kind == active_kind
            {
                if let Some(held) = self.messages.iter_mut().find(|held| held.id == id) {
                    let before = weight(&held.message);
                    if held.append(self.sequence, &incoming.message) {
                        self.bytes = self
                            .bytes
                            .saturating_sub(before)
                            .saturating_add(weight(&held.message));
                        next = None;
                    }
                } else {
                    next = None;
                }
            }
        }
        if let Some(next) = next {
            if next.tool_id().is_some() {
                self.continuation = None;
            }
            if let Some(kind) = streamed {
                self.continuation = Some((next.id, kind));
            }
            self.bytes = self.bytes.saturating_add(weight(&next.message));
            self.messages.push(next);
        }
        while self.messages.len() > MAX_MESSAGES || self.bytes > MAX_BYTES {
            let removed = self.messages.remove(0);
            self.retained_from = self.retained_from.max(removed.sequence.saturating_add(1));
            self.bytes = self.bytes.saturating_sub(weight(&removed.message));
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
    draft_writer: std::sync::OnceLock<std::sync::Arc<super::session_drafts::DraftWriter>>,
    closing: Notify,
    reader: std::sync::OnceLock<magistr_engine::NativeReader>,
    source: std::sync::OnceLock<std::sync::Weak<magistr_workspace::service::WorkspaceService>>,
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
                attachments: Vec::new(),
                draft_revision: 0,
                draft_saved_revision: 0,
                draft_save_attempt: 0,
                draft_save_error: None,
                admission: None,
                activity: None,
                bytes: 0,
                retained_from: 1,
                continuation: None,
                answered: std::collections::BTreeSet::new(),
                seed_at: 0,
                usage: None,
            })
            .0,
            closed: AtomicBool::new(false),
            draft_writer: std::sync::OnceLock::new(),
            closing: Notify::new(),
            reader: std::sync::OnceLock::new(),
            source: std::sync::OnceLock::new(),
            wake: std::sync::OnceLock::new(),
        }
    }

    pub(super) fn seed(&self, seed: ProjectSeed) {
        self.state.send_modify(|state| {
            let draft = state.seed.draft.take();
            state.seed = seed;
            if draft.is_some() {
                state.seed.draft = draft;
            }
        });
        self.wake_catalog();
    }
    pub(super) fn draft(&self, text: String) {
        self.state.send_modify(|state| {
            if state.seed.draft.as_ref() != Some(&text) {
                state.record(Msg::PromptReplace(text));
            }
        });
        self.persist_draft();
    }

    pub(super) async fn attach_history(
        &self,
        history: crate::agent::persistence::SessionHistory,
    ) -> Result<(), String> {
        let saved = history.load_draft().await?;
        self.state.send_modify(|state| {
            if state.draft_revision == 0 {
                state.seed.draft = Some(saved.text);
                state.attachments = saved.attachments;
                state.draft_revision = saved.status.revision;
                state.draft_saved_revision = saved.status.saved_revision;
                state.draft_save_attempt = saved.status.save_attempt;
            }
        });
        let writer = super::session_drafts::DraftWriter::start(history, self.state.clone());
        self.draft_writer
            .set(writer)
            .map_err(|_| "This conversation already has a draft owner.".to_owned())?;
        self.persist_draft();
        Ok(())
    }

    pub(super) fn edit_draft(
        &self,
        expected: u64,
        text: String,
        discard: bool,
    ) -> Result<u64, String> {
        let mut result = Ok(expected);
        self.state.send_if_modified(|state| {
            if state.closed {
                result = Err("This session is no longer open.".into());
                return false;
            }
            if state.draft_revision != expected {
                result = Err(crate::agent::persistence::draft::STALE.into());
                return false;
            }
            let changed = state.seed.draft.as_ref() != Some(&text)
                || (discard && !state.attachments.is_empty());
            if !changed {
                return false;
            }
            state.seed.draft = Some(text);
            if discard {
                state.attachments.clear();
            }
            state.draft_revision = state.draft_revision.saturating_add(1);
            result = Ok(state.draft_revision);
            state.sequence = state.sequence.saturating_add(1);
            true
        });
        if result.is_ok() {
            self.persist_draft();
        }
        result
    }

    fn persist_draft(&self) {
        if let Some(writer) = self.draft_writer.get() {
            writer.save(super::SavedDraft::from_snapshot(&self.state.borrow()));
        }
    }

    pub(super) async fn flush_draft(&self) -> super::DraftStatus {
        match self.draft_writer.get() {
            Some(writer) => writer.flush().await,
            None => self.state.borrow().draft_status(),
        }
    }
    pub(super) fn add_attachments(
        &self,
        attachments: Vec<super::StagedAttachment>,
    ) -> Result<(), String> {
        let mut result = Ok(());
        self.state.send_if_modified(|state| {
            if state.closed {
                result = Err("This session is no longer open.".into());
                return false;
            }
            let input = PromptInput {
                text: state.seed.draft.clone().unwrap_or_default(),
                attachments: state
                    .attachments
                    .iter()
                    .chain(&attachments)
                    .map(|attachment| attachment.content().clone())
                    .collect(),
            };
            if let Err(error) = input.validate(true) {
                result = Err(error);
                return false;
            }
            if attachments.is_empty() {
                return false;
            }
            state.attachments.extend(attachments);
            state.draft_revision = state.draft_revision.saturating_add(1);
            state.sequence = state.sequence.saturating_add(1);
            true
        });
        if result.is_ok() {
            self.persist_draft();
        }
        result
    }
    pub(super) fn remove_attachment(&self, id: &str) -> Result<(), String> {
        let mut found = false;
        self.state.send_if_modified(|state| {
            if state.closed {
                return false;
            }
            let Some(at) = state
                .attachments
                .iter()
                .position(|attachment| attachment.id() == id)
            else {
                return false;
            };
            state.attachments.remove(at);
            state.draft_revision = state.draft_revision.saturating_add(1);
            state.sequence = state.sequence.saturating_add(1);
            found = true;
            true
        });
        if found {
            self.persist_draft();
            Ok(())
        } else {
            Err("This attachment is no longer in the message.".into())
        }
    }
    pub(super) fn set_source(
        &self,
        source: std::sync::Weak<magistr_workspace::service::WorkspaceService>,
    ) {
        let _ = self.source.set(source);
    }
    pub(super) fn source(
        &self,
    ) -> Option<std::sync::Arc<magistr_workspace::service::WorkspaceService>> {
        self.source.get()?.upgrade()
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
        let mut draft_changed = false;
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
            let revision = state.draft_revision;
            let configuration = matches!(&message, Msg::Agent(AgentEvent::Conversation(_) | AgentEvent::Configuration { .. }));
            state.record(message);
            draft_changed = configuration || state.draft_revision != revision;
            catalog_changed = before.is_some_and(|before| before != super::session_catalog::SessionSummary::from_snapshot(state));
        });
        if draft_changed {
            self.persist_draft();
        }
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
