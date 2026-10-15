//! The desktop's bounded, typed view of application observations and their identity.
use crate::content::{Observation, observation};
use base64::Engine as _;
use magistr::application::{ConversationTarget, SessionCatalog, SessionSnapshot, SessionSummary};
use magistr_application::sessions::{AgentEvent, PermissionKind, SessionPurpose};
use magistr_cockpit::cockpit::Msg;
use magistr_domain::identity::SessionAddress;
use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct Frame {
    pub(crate) stream: u64,
    pub(crate) address: SessionAddress,
    pub(crate) target: Option<ConversationTarget>,
    pub(crate) sequence: u64,
    pub(crate) workflow_revision: u64,
    pub(crate) reseed: bool,
    pub(crate) dropped: u64,
    pub(crate) events: Vec<Observation>,
    pub(crate) retained: Vec<u64>,
    pub(crate) pending: Vec<Request>,
    pub(crate) draft: String,
    pub(crate) draft_revision: u64,
    pub(crate) draft_saved_revision: u64,
    pub(crate) draft_save_attempt: u64,
    pub(crate) draft_save_error: Option<String>,
    pub(crate) attachments: Vec<Attachment>,
    pub(crate) status: String,
    pub(crate) model: Option<String>,
    pub(crate) usage: Option<Usage>,
    pub(crate) closed: bool,
    pub(crate) activity: Option<Activity>,
    pub(crate) conversation: Option<Conversation>,
    pub(crate) configuration: Vec<Configuration>,
    pub(crate) admissions: Vec<Admission>,
    pub(crate) work: Option<magistr_cockpit::cockpit::WorkRun>,
    pub(crate) work_admission: Option<magistr::application::WorkAdmission>,
    pub(crate) work_error: Option<String>,
}
#[derive(Serialize)]
pub(crate) struct Conversation {
    id: String,
    name: Option<String>,
    provider: String,
    provider_label: String,
    purpose: &'static str,
}
impl From<&magistr_application::sessions::Conversation> for Conversation {
    fn from(value: &magistr_application::sessions::Conversation) -> Self {
        Self {
            id: value.id.clone(),
            purpose: purpose(value.purpose),
            name: value.name.clone(),
            provider: value.provider.clone(),
            provider_label: magistr::agent::AgentRuntime::provider_label_for(&value.provider)
                .to_owned(),
        }
    }
}
pub(crate) fn purpose(value: SessionPurpose) -> &'static str {
    match value {
        SessionPurpose::Conversation => "conversation",
        SessionPurpose::Roadmap => "roadmap",
    }
}
#[derive(Serialize)]
pub(crate) struct Configuration {
    id: String,
    name: String,
    current_value: String,
    values: Vec<(String, String)>,
}
impl From<&magistr_application::sessions::SessionConfig> for Configuration {
    fn from(value: &magistr_application::sessions::SessionConfig) -> Self {
        Self {
            id: value.id.clone(),
            name: value.name.clone(),
            current_value: value.current_value.clone(),
            values: value.values.clone(),
        }
    }
}
#[derive(Serialize)]
pub(crate) struct Admission {
    sequence: u64,
    text: String,
    attachments: Vec<AttachmentIdentity>,
    accepted: bool,
}

#[derive(Serialize)]
struct AttachmentIdentity {
    id: String,
    name: String,
    content_hash: String,
}

#[derive(Serialize)]
pub(crate) struct Attachment {
    pub(crate) id: String,
    name: String,
    content_hash: String,
    path: Option<String>,
    size: usize,
    mime_type: Option<&'static str>,
    preview: Option<String>,
    excerpt: Option<String>,
}

impl Attachment {
    pub(crate) fn from_staged(
        file: &magistr::application::StagedAttachment,
        seen: &[String],
    ) -> Self {
        Self::from_content(file.content(), file.path(), seen)
    }

    fn from_content(
        content: &magistr_application::sessions::PromptAttachment,
        path: Option<&str>,
        seen: &[String],
    ) -> Self {
        let id = content.id();
        let fresh = !seen.iter().any(|seen| seen == id);
        Self {
            id: id.into(),
            name: content.name().into(),
            content_hash: content.content_hash().into(),
            path: path.map(str::to_owned),
            size: content.bytes().len(),
            mime_type: content.mime_type(),
            preview: content.mime_type().filter(|_| fresh).map(|mime| {
                format!(
                    "data:{mime};base64,{}",
                    base64::engine::general_purpose::STANDARD.encode(content.bytes())
                )
            }),
            excerpt: content
                .text()
                .filter(|_| fresh)
                .map(|text| text.chars().take(240).collect()),
        }
    }
}
impl From<&magistr::application::SubmissionAdmission> for Admission {
    fn from(admission: &magistr::application::SubmissionAdmission) -> Self {
        Self {
            sequence: admission.sequence,
            text: admission.input.text.clone(),
            attachments: admission
                .input
                .attachments
                .iter()
                .map(|file| AttachmentIdentity {
                    id: file.id.clone(),
                    name: file.name.clone(),
                    content_hash: file.content_hash.clone(),
                })
                .collect(),
            accepted: admission.accepted,
        }
    }
}
#[derive(Serialize)]
pub(crate) struct Request {
    id: u64,
    title: String,
    detail: String,
    choices: Vec<Choice>,
}
#[derive(Serialize)]
struct Choice {
    option_id: String,
    label: &'static str,
}
#[derive(Serialize)]
pub(crate) struct Activity {
    active: bool,
    queued_turns: usize,
    configuration_in_progress: bool,
    cancellation_requested: bool,
    can_resume: Option<bool>,
    can_send_images: Option<bool>,
}
impl From<&magistr_application::sessions::SessionActivity> for Activity {
    fn from(activity: &magistr_application::sessions::SessionActivity) -> Self {
        Self {
            active: activity.active,
            queued_turns: activity.queued_turns,
            configuration_in_progress: activity.configuration_in_progress,
            cancellation_requested: activity.cancellation_requested,
            can_resume: activity.can_resume,
            can_send_images: activity.can_send_images,
        }
    }
}
#[derive(Serialize)]
pub(crate) struct CatalogFrame {
    pub(crate) stream: u64,
    pub(crate) sequence: u64,
    pub(crate) sessions: Vec<SessionInfo>,
}
#[derive(Serialize)]
pub(crate) struct SessionInfo {
    address: SessionAddress,
    target: Option<ConversationTarget>,
    owned_conversations: Vec<String>,
    work: Option<magistr_cockpit::cockpit::WorkRun>,
    name: String,
    conversation: Option<Conversation>,
    status: String,
    model: Option<String>,
    usage: Option<Usage>,
    activity: Option<Activity>,
    pending: Vec<Waiting>,
    closed: bool,
}
#[derive(Serialize)]
struct Waiting {
    id: u64,
    title: String,
}
impl From<&SessionSummary> for SessionInfo {
    fn from(summary: &SessionSummary) -> Self {
        Self {
            address: summary.address.clone(),
            target: summary
                .conversation
                .as_ref()
                .map(|conversation| ConversationTarget {
                    address: summary.address.clone(),
                    conversation: conversation.id.clone(),
                }),
            owned_conversations: summary.owned_conversations.clone(),
            work: summary.work.clone(),
            name: summary.name.clone(),
            conversation: summary.conversation.as_ref().map(Conversation::from),
            status: summary.label().into(),
            model: summary.model.clone(),
            usage: usage(summary.usage.as_ref()),
            activity: summary.activity.as_ref().map(Activity::from),
            pending: summary
                .requests
                .iter()
                .map(|request| Waiting {
                    id: request.id,
                    title: request.title.clone(),
                })
                .collect(),
            closed: summary.closed,
        }
    }
}
impl CatalogFrame {
    pub(crate) fn from_catalog(stream: u64, catalog: &SessionCatalog) -> Self {
        Self {
            stream,
            sequence: catalog.sequence,
            sessions: catalog.sessions.iter().map(SessionInfo::from).collect(),
        }
    }
}
fn usage(message: Option<&Msg>) -> Option<Usage> {
    match message {
        Some(Msg::Agent(AgentEvent::Usage {
            context,
            cost_cents,
            ..
        })) => Some(Usage::from_report(*context, *cost_cents)),
        _ => None,
    }
}
#[derive(Serialize)]
pub(crate) struct Usage {
    used: Option<u64>,
    size: Option<u64>,
    cost_cents: Option<u64>,
}

impl Usage {
    fn from_report(
        context: Option<magistr_application::sessions::ContextUsage>,
        cost_cents: Option<u64>,
    ) -> Self {
        Self {
            used: context.map(|context| context.used),
            size: context.map(|context| context.size),
            cost_cents,
        }
    }
}

impl Frame {
    pub(crate) fn from_snapshot(
        stream: u64,
        snapshot: &SessionSnapshot,
        seen: Option<u64>,
        attachments_seen: &[String],
    ) -> Self {
        let incremental = seen.and_then(|at| snapshot.since(at));
        let reseed = incremental.is_none();
        let events = incremental
            .unwrap_or_else(|| snapshot.messages.clone())
            .iter()
            .filter_map(observation)
            .collect();
        let pending = snapshot.pending_requests().filter_map(request).collect();
        let status = SessionSummary::from(snapshot).label().to_owned();
        let model = snapshot.model.clone();
        let usage = usage(snapshot.latest_usage());
        Self {
            stream,
            address: snapshot.address.clone(),
            target: snapshot.target(),
            sequence: snapshot.sequence,
            workflow_revision: snapshot.workflow_revision,
            reseed,
            dropped: snapshot.dropped,
            events,
            retained: snapshot.messages.iter().map(|record| record.id).collect(),
            pending,
            draft: snapshot.seed.draft.clone().unwrap_or_default(),
            draft_revision: snapshot.draft_revision,
            draft_saved_revision: snapshot.draft_saved_revision,
            draft_save_attempt: snapshot.draft_save_attempt,
            draft_save_error: snapshot.draft_save_error.clone(),
            attachments: snapshot
                .attachments
                .iter()
                .map(|file| Attachment::from_staged(file, attachments_seen))
                .collect(),
            status,
            model,
            usage,
            closed: snapshot.closed,
            work: snapshot.work.clone(),
            work_admission: snapshot.work_admission.clone(),
            work_error: snapshot.work_error.clone(),
            activity: snapshot.activity.as_ref().map(Activity::from),
            conversation: snapshot.conversation.as_ref().map(Conversation::from),
            configuration: snapshot
                .configuration
                .iter()
                .map(Configuration::from)
                .collect(),
            admissions: snapshot
                .admission
                .as_ref()
                .filter(|admission| admission.sequence > seen.unwrap_or(0))
                .map(Admission::from)
                .into_iter()
                .collect(),
        }
    }
}

fn request(message: &Msg) -> Option<Request> {
    let Msg::Agent(AgentEvent::PermissionRequest {
        id,
        title,
        detail,
        choices,
        ..
    }) = message
    else {
        return None;
    };
    Some(Request {
        id: *id,
        title: title.clone(),
        detail: detail.clone(),
        choices: choices
            .iter()
            .map(|choice| Choice {
                option_id: choice.option_id.clone(),
                label: match choice.kind {
                    PermissionKind::AllowOnce => "Allow once",
                    PermissionKind::AllowAlways => "Always allow",
                    PermissionKind::RejectOnce => "Deny",
                    PermissionKind::RejectAlways => "Always deny",
                },
            })
            .collect(),
    })
}
#[cfg(test)]
#[path = "wire.tests.rs"]
mod tests;
