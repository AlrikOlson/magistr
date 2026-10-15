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
    /// Changes only when the owner publishes different workflow records or provenance.
    pub workflow_revision: u64,
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
    pub work: Option<magistr_cockpit::cockpit::WorkRun>,
    pub work_admission: Option<super::WorkAdmission>,
    pub work_error: Option<String>,
    bytes: usize,
    retained_from: u64,
    // An evicted active source keeps its kind/id so its tail cannot become a new reply.
    continuation: Option<(u64, super::retained_observation::StreamKind)>,
    answered: std::collections::BTreeSet<u64>,
    usage: Option<Msg>,
}

impl SessionSnapshot {
    #[must_use]
    pub fn target(&self) -> Option<super::ConversationTarget> {
        self.conversation
            .as_ref()
            .map(|conversation| super::ConversationTarget {
                address: self.address.clone(),
                conversation: conversation.id.clone(),
            })
    }
    #[must_use]
    pub fn draft_status(&self) -> super::DraftStatus {
        super::DraftStatus {
            conversation: self
                .conversation
                .as_ref()
                .map(|conversation| conversation.id.clone())
                .unwrap_or_default(),
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
                    !matches!(
                        msg,
                        Msg::Agent(AgentEvent::PermissionRequest { .. } | AgentEvent::Usage { .. })
                    )
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
            Msg::CommandFailed {
                cmd: magistr_cockpit::cockpit::Cmd::Launch(_),
                error,
            } => {
                self.work_admission = Some(super::WorkAdmission {
                    sequence: self.sequence,
                    run_id: None,
                    error: Some(error.clone()),
                });
            }
            Msg::CommandFailed {
                cmd: magistr_cockpit::cockpit::Cmd::Loop(_),
                error,
            } => {
                self.work_error = Some(error.clone());
            }
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

#[derive(Debug, Default)]
struct Conversations {
    active: Option<String>,
    parked: BTreeMap<String, SessionSnapshot>,
    writers: BTreeMap<String, std::sync::Arc<super::session_drafts::DraftWriter>>,
}

#[derive(Debug)]
pub(crate) struct SessionObservation {
    state: watch::Sender<SessionSnapshot>,
    conversations: std::sync::Mutex<Conversations>,
    closed: AtomicBool,
    closing: Notify,
    reader: std::sync::OnceLock<super::workflow_client::SnapshotReads>,
    snapshot_requests: std::sync::OnceLock<super::workflow_client::FeedLink>,
    source: std::sync::OnceLock<std::sync::Weak<magistr_workspace::service::WorkspaceService>>,
    wake: std::sync::OnceLock<std::sync::Arc<Notify>>,
}

fn empty_snapshot(address: SessionAddress) -> SessionSnapshot {
    SessionSnapshot {
        address,
        sequence: 0,
        workflow_revision: 0,
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
        work: None,
        work_admission: None,
        work_error: None,
        bytes: 0,
        retained_from: 1,
        continuation: None,
        answered: std::collections::BTreeSet::new(),
        usage: None,
    }
}

impl SessionObservation {
    pub(crate) fn new(address: SessionAddress) -> Self {
        Self {
            state: watch::channel(empty_snapshot(address)).0,
            conversations: std::sync::Mutex::default(),
            closed: AtomicBool::new(false),
            closing: Notify::new(),
            reader: std::sync::OnceLock::new(),
            snapshot_requests: std::sync::OnceLock::new(),
            source: std::sync::OnceLock::new(),
            wake: std::sync::OnceLock::new(),
        }
    }

    /// One canonical retained snapshot per conversation: the active snapshot is
    /// in the watch sender, and parked snapshots are moved into this owner's map.
    fn modify<R>(
        &self,
        id: Option<&str>,
        require_active: bool,
        change: impl FnOnce(&mut SessionSnapshot) -> R,
    ) -> Result<R, String> {
        let mut conversations = self
            .conversations
            .lock()
            .expect("conversation state lock poisoned");
        if require_active && conversations.active.as_deref() != id {
            return Err(changed_conversation());
        }
        if let Some(id) = id
            && conversations.active.as_deref() != Some(id)
        {
            let snapshot = conversations
                .parked
                .get_mut(id)
                .ok_or_else(changed_conversation)?;
            return Ok(change(snapshot));
        }
        let mut change = Some(change);
        let mut result = None;
        self.state.send_modify(|state| {
            result = Some(change.take().expect("one state mutation")(state));
        });
        Ok(result.expect("state mutation ran"))
    }

    #[cfg(test)]
    pub(super) fn bind_context_for_test(
        &self,
        conversation: magistr_application::sessions::Conversation,
    ) {
        let id = conversation.id.clone();
        let mut snapshot = empty_snapshot(self.state.borrow().address.clone());
        snapshot.conversation = Some(conversation);
        self.conversations
            .lock()
            .expect("conversation state lock poisoned")
            .parked
            .insert(id.clone(), snapshot);
        self.activate_context(&id)
            .expect("fixture context activates");
    }

    pub(crate) fn owns(&self, id: &str) -> bool {
        let contexts = self
            .conversations
            .lock()
            .expect("conversation state lock poisoned");
        contexts.active.as_deref() == Some(id) || contexts.parked.contains_key(id)
    }

    pub(super) fn validate_target(&self, target: &super::ConversationTarget) -> Result<(), String> {
        let contexts = self
            .conversations
            .lock()
            .expect("conversation state lock poisoned");
        if self.is_closed()
            || self.state.borrow().address != target.address
            || (contexts.active.as_deref() != Some(&target.conversation)
                && !contexts.parked.contains_key(&target.conversation))
        {
            return Err(changed_conversation());
        }
        Ok(())
    }

    pub(super) fn validate_active(&self, target: &super::ConversationTarget) -> Result<(), String> {
        self.validate_target(target)?;
        let contexts = self
            .conversations
            .lock()
            .expect("conversation state lock poisoned");
        if contexts.active.as_deref() != Some(&target.conversation) {
            return Err(changed_conversation());
        }
        Ok(())
    }

    pub(crate) async fn bind_history(
        self: &std::sync::Arc<Self>,
        history: crate::agent::persistence::SessionHistory,
    ) -> Result<(), String> {
        let id = history.id();
        if self.owns(&id) {
            return Ok(());
        }
        let conversation = history.conversation().ok_or_else(|| {
            "Wait for the conversation to open before saving a message.".to_owned()
        })?;
        let saved = history.load_draft().await?;
        let mut snapshot = empty_snapshot(self.state.borrow().address.clone());
        snapshot.conversation = Some(conversation);
        snapshot.model = history.current_model();
        snapshot.seed.draft = Some(saved.text);
        snapshot.attachments = saved.attachments;
        snapshot.draft_revision = saved.status.revision;
        snapshot.draft_saved_revision = saved.status.saved_revision;
        snapshot.draft_save_attempt = saved.status.save_attempt;
        let initial = snapshot.draft_status();
        let weak = std::sync::Arc::downgrade(self);
        let completed_id = id.clone();
        let writer = super::session_drafts::DraftWriter::start(history, initial, move |status| {
            if let Some(owner) = weak.upgrade() {
                let _ = owner.modify(Some(&completed_id), false, |snapshot| {
                    snapshot.draft_saved_revision = status.saved_revision;
                    snapshot.draft_save_attempt = status.save_attempt;
                    snapshot.draft_save_error.clone_from(&status.error);
                    snapshot.sequence = snapshot.sequence.saturating_add(1);
                });
            }
        });
        {
            let mut contexts = self
                .conversations
                .lock()
                .expect("conversation state lock poisoned");
            if contexts.active.as_deref() == Some(&id) || contexts.parked.contains_key(&id) {
                return Ok(());
            }
            contexts.parked.insert(id.clone(), snapshot);
            contexts.writers.insert(id.clone(), writer);
        }
        self.persist_draft_for(Some(&id));
        self.wake_catalog();
        Ok(())
    }

    pub(crate) fn activate_context(&self, id: &str) -> Result<(), String> {
        let mut contexts = self
            .conversations
            .lock()
            .expect("conversation state lock poisoned");
        if self.is_closed() {
            return Err(changed_conversation());
        }
        if contexts.active.as_deref() == Some(id) {
            return Ok(());
        }
        let mut next = contexts
            .parked
            .remove(id)
            .ok_or_else(changed_conversation)?;
        let previous_id = contexts.active.replace(id.to_owned());
        self.state.send_modify(|current| {
            let draft = next.seed.draft.take();
            next.seed = current.seed.clone();
            next.workflow_revision = current.workflow_revision;
            next.work = current.work.clone();
            next.work_admission = current.work_admission.clone();
            next.work_error = current.work_error.clone();
            next.seed.draft = draft;
            next.sequence = current.sequence.max(next.sequence).saturating_add(1);
            // A switch is a reseed even when both contexts reused source IDs.
            next.retained_from = next.sequence.saturating_add(1);
            let previous = std::mem::replace(current, next.clone());
            if let Some(previous_id) = previous_id {
                contexts.parked.insert(previous_id, previous);
            }
        });
        drop(contexts);
        self.wake_catalog();
        Ok(())
    }

    pub(super) fn seed(&self, seed: ProjectSeed) {
        let _ = self.modify(None, false, |state| {
            let draft = state.seed.draft.take();
            let observed = std::mem::take(&mut state.seed.snapshot);
            let changed = observed.as_ref() == &magistr_engine::NativeSnapshot::default()
                && observed != seed.snapshot;
            state.seed = seed;
            if observed.as_ref() != &magistr_engine::NativeSnapshot::default() {
                state.seed.snapshot = observed;
            }
            if changed {
                state.workflow_revision = state.workflow_revision.saturating_add(1);
            }
            if draft.is_some() {
                state.seed.draft = draft;
            }
        });
        self.wake_catalog();
    }

    pub(super) fn draft_for(&self, id: &str, text: String) -> Result<(), String> {
        self.modify(Some(id), true, |state| {
            if state.seed.draft.as_ref() != Some(&text) {
                state.record(Msg::PromptReplace(text));
            }
        })?;
        self.persist_draft_for(Some(id));
        Ok(())
    }

    pub(super) fn edit_draft_for(
        &self,
        id: &str,
        expected: u64,
        text: String,
        discard: bool,
    ) -> Result<u64, String> {
        self.edit_draft_in(Some(id), expected, text, discard)
    }

    fn edit_draft_in(
        &self,
        id: Option<&str>,
        expected: u64,
        text: String,
        discard: bool,
    ) -> Result<u64, String> {
        let result = self.modify(id, false, |state| {
            if state.closed {
                return Err("This session is no longer open.".into());
            }
            if state.draft_revision != expected {
                return Err(crate::agent::persistence::draft::STALE.into());
            }
            if state.seed.draft.as_ref() != Some(&text)
                || (discard && !state.attachments.is_empty())
            {
                state.seed.draft = Some(text);
                if discard {
                    state.attachments.clear();
                }
                state.draft_revision = state.draft_revision.saturating_add(1);
                state.sequence = state.sequence.saturating_add(1);
            }
            Ok(state.draft_revision)
        })?;
        if result.is_ok() {
            self.persist_draft_for(id);
        }
        result
    }

    fn persist_draft_for(&self, id: Option<&str>) {
        let contexts = self
            .conversations
            .lock()
            .expect("conversation state lock poisoned");
        let Some(id) = id.or(contexts.active.as_deref()) else {
            return;
        };
        let Some(writer) = contexts.writers.get(id) else {
            return;
        };
        let snapshot = if contexts.active.as_deref() == Some(id) {
            self.state.borrow().clone()
        } else if let Some(snapshot) = contexts.parked.get(id) {
            snapshot.clone()
        } else {
            return;
        };
        writer.save(super::SavedDraft::from_snapshot(&snapshot));
    }

    pub(super) async fn flush_draft_for(&self, id: &str) -> super::DraftStatus {
        let writer = self
            .conversations
            .lock()
            .expect("conversation state lock poisoned")
            .writers
            .get(id)
            .cloned();
        match writer {
            Some(writer) => writer.flush().await,
            None => {
                let mut status = self
                    .context_snapshot(id)
                    .map(|snapshot| snapshot.draft_status())
                    .unwrap_or_default();
                status.conversation = id.to_owned();
                status
            }
        }
    }
    pub(crate) async fn flush_all(&self) -> Result<(), String> {
        let writers: Vec<_> = self
            .conversations
            .lock()
            .expect("conversation state lock poisoned")
            .writers
            .values()
            .cloned()
            .collect();
        let mut errors = Vec::new();
        for writer in writers {
            if let Some(error) = writer.flush().await.error {
                errors.push(error);
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("\n"))
        }
    }

    pub(super) fn add_attachments_for(
        &self,
        id: &str,
        attachments: Vec<super::StagedAttachment>,
    ) -> Result<(), String> {
        self.add_attachments_in(Some(id), attachments)
    }
    fn add_attachments_in(
        &self,
        id: Option<&str>,
        attachments: Vec<super::StagedAttachment>,
    ) -> Result<(), String> {
        let result = self.modify(id, false, |state| {
            if state.closed {
                return Err("This session is no longer open.".into());
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
            input.validate(true)?;
            if !attachments.is_empty() {
                state.attachments.extend(attachments);
                state.draft_revision = state.draft_revision.saturating_add(1);
                state.sequence = state.sequence.saturating_add(1);
            }
            Ok(())
        })?;
        if result.is_ok() {
            self.persist_draft_for(id);
        }
        result
    }

    pub(super) fn remove_attachment_for(&self, conversation: &str, id: &str) -> Result<(), String> {
        self.remove_attachment_in(Some(conversation), id)
    }
    fn remove_attachment_in(&self, conversation: Option<&str>, id: &str) -> Result<(), String> {
        let found = self.modify(conversation, false, |state| {
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
            true
        })?;
        if found {
            self.persist_draft_for(conversation);
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
    pub(crate) fn snapshot(&self) -> SessionSnapshot {
        self.state.borrow().clone()
    }
    pub(crate) fn context_snapshot(&self, id: &str) -> Option<SessionSnapshot> {
        let contexts = self
            .conversations
            .lock()
            .expect("conversation state lock poisoned");
        if contexts.active.as_deref() == Some(id) {
            Some(self.state.borrow().clone())
        } else {
            contexts.parked.get(id).cloned().map(|mut snapshot| {
                let current = self.state.borrow();
                snapshot.work = current.work.clone();
                snapshot.work_admission = current.work_admission.clone();
                snapshot.work_error = current.work_error.clone();
                snapshot
            })
        }
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
        let contexts = self
            .conversations
            .lock()
            .expect("conversation state lock poisoned");
        let mut summary =
            super::session_catalog::SessionSummary::from_snapshot(&self.state.borrow());
        summary.owned_conversations = contexts
            .parked
            .keys()
            .chain(contexts.active.iter())
            .cloned()
            .collect();
        summary.owned_conversations.sort();
        summary
    }
    pub(super) fn set_reader(
        &self,
        reader: super::workflow_client::SnapshotReads,
        requests: super::workflow_client::FeedLink,
    ) {
        let _ = self.reader.set(reader);
        let _ = self.snapshot_requests.set(requests);
    }
    pub(super) fn subscribe(self: &std::sync::Arc<Self>) -> SessionSubscription {
        SessionSubscription {
            receiver: self.state.subscribe(),
            observation: std::sync::Arc::clone(self),
        }
    }
    pub(super) fn is_answered(&self, conversation: &str, id: u64) -> bool {
        self.context_snapshot(conversation)
            .is_some_and(|snapshot| snapshot.answered.contains(&id))
    }
    pub(super) fn permission_answered_for(
        &self,
        conversation: &str,
        id: u64,
    ) -> Result<(), String> {
        self.modify(Some(conversation), false, |state| {
            if state.answered.insert(id) {
                state.sequence = state.sequence.saturating_add(1);
            }
        })?;
        self.wake_catalog();
        Ok(())
    }
    pub(super) fn stop(&self) {
        self.closed.store(true, Ordering::Release);
        let mut contexts = self
            .conversations
            .lock()
            .expect("conversation state lock poisoned");
        for snapshot in contexts.parked.values_mut() {
            snapshot.closed = true;
        }
        self.state.send_modify(|state| {
            state.closed = true;
            state.sequence = state.sequence.saturating_add(1);
        });
        drop(contexts);
        self.closing.notify_waiters();
        self.wake_catalog();
    }

    fn record_for(&self, id: Option<&str>, message: Msg) -> bool {
        if self.is_closed() {
            return false;
        }
        let mut overloaded = false;
        let mut draft_changed = false;
        let result = self.modify(id, false, |state| {
            if state.closed { return; }
            if let Msg::Agent(AgentEvent::Conversation(conversation)) = &message
                && id.is_some_and(|id| id != conversation.id) { return; }
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
        });
        if result.is_err() {
            return false;
        }
        if draft_changed {
            self.persist_draft_for(id);
        }
        self.wake_catalog();
        if overloaded {
            self.stop();
        }
        !overloaded
    }
}

fn changed_conversation() -> String {
    "The conversation changed. Review the current conversation and try again.".into()
}

impl ObservationSink for SessionObservation {
    fn activate(&self, id: &str) -> Result<(), String> {
        self.activate_context(id)
    }
    fn send_for(&self, id: &str, message: Msg) -> bool {
        if matches!(
            &message,
            Msg::Engine(_)
                | Msg::WorkRun(_)
                | Msg::CommandFailed {
                    cmd: magistr_cockpit::cockpit::Cmd::Launch(_)
                        | magistr_cockpit::cockpit::Cmd::Loop(_),
                    ..
                }
        ) {
            return self.send(message);
        }
        self.record_for(Some(id), message)
    }
    fn send(&self, message: Msg) -> bool {
        if self.is_closed() {
            return false;
        }
        if let Msg::WorkRun(work) = message {
            let _ = self.modify(None, false, |state| {
                state.sequence = state.sequence.saturating_add(1);
                if work.is_active()
                    && state
                        .work
                        .as_ref()
                        .is_none_or(|previous| previous.id != work.id || !previous.is_active())
                {
                    state.work_admission = Some(super::WorkAdmission {
                        sequence: state.sequence,
                        run_id: Some(work.id.clone()),
                        error: None,
                    });
                }
                state.work = Some(work);
                state.work_error = None;
            });
            self.wake_catalog();
            return true;
        }
        if let Msg::Engine(event) = message {
            let publishing = matches!(&event, magistr_cockpit::engine::EngineEvent::Seed(_));
            let apply = || {
                let mut requested = false;
                let _ = self.modify(None, false, |state| {
                    if state.closed {
                        return;
                    }
                    let before = state.seed.snapshot.clone();
                    requested = super::session_workflow::observe(&mut state.seed.snapshot, event);
                    if !std::sync::Arc::ptr_eq(&before, &state.seed.snapshot) {
                        state.workflow_revision = state.workflow_revision.saturating_add(1);
                        state.sequence = state.sequence.saturating_add(1);
                    }
                });
                requested
            };
            let requested = if !publishing && let Some(reader) = self.reader.get() {
                reader.invalidate(apply)
            } else {
                apply()
            };
            if requested && let Some(link) = self.snapshot_requests.get() {
                link.request_snapshot();
            }
            return true;
        }
        self.record_for(None, message)
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
            let inbox = magistr_cockpit_runtime::channel::Inbox::observed(self.observation.clone());
            tokio::select! {
                () = inbox.closed() => {},
                _ = reader.read(Some(&inbox), true) => {},
            }
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

#[cfg(test)]
#[path = "session_context.tests.rs"]
mod context_tests;
