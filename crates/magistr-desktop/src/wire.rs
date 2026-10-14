//! The desktop's bounded, typed view of application observations and their identity.
use magistr::application::{SessionCatalog, SessionSnapshot, SessionSummary};
use magistr_application::sessions::{AgentEvent, PermissionKind, ToolStatus, TurnEnd};
use magistr_cockpit::cockpit::Msg;
use magistr_domain::identity::SessionAddress;
use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct Frame {
    pub(crate) stream: u64,
    pub(crate) address: SessionAddress,
    pub(crate) sequence: u64,
    pub(crate) reseed: bool,
    pub(crate) dropped: u64,
    pub(crate) events: Vec<Observation>,
    pub(crate) pending: Vec<Request>,
    pub(crate) draft: String,
    pub(crate) status: String,
    pub(crate) model: Option<String>,
    pub(crate) usage: Option<Usage>,
    pub(crate) closed: bool,
    pub(crate) activity: Option<Activity>,
}
#[derive(Serialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub(crate) enum Observation {
    User(String),
    Text(String),
    Thought(String),
    Tool(String),
    Ended(String),
    Notice(String),
    Error(String),
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
    cancellation_requested: bool,
    can_resume: Option<bool>,
    can_send_images: Option<bool>,
}
impl From<&magistr_application::sessions::SessionActivity> for Activity {
    fn from(activity: &magistr_application::sessions::SessionActivity) -> Self {
        Self {
            active: activity.active,
            queued_turns: activity.queued_turns,
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
    name: String,
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
            name: summary.name.clone(),
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
    ) -> Self {
        let incremental = seen.and_then(|at| snapshot.since(at));
        let reseed = incremental.is_none();
        let events = incremental
            .unwrap_or_else(|| snapshot.replay())
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
            sequence: snapshot.sequence,
            reseed,
            dropped: snapshot.dropped,
            events,
            pending,
            draft: snapshot.seed.draft.clone().unwrap_or_default(),
            status,
            model,
            usage,
            closed: snapshot.closed,
            activity: snapshot.activity.as_ref().map(Activity::from),
        }
    }
}

fn observation(message: &Msg) -> Option<Observation> {
    Some(match message {
        Msg::InputAccepted(text) => Observation::User(text.clone()),
        Msg::CommandFailed { error, .. } => Observation::Error(error.clone()),
        Msg::Agent(event) => match event {
            AgentEvent::Text(text) => Observation::Text(text.clone()),
            AgentEvent::Thought(text) => Observation::Thought(text.clone()),
            AgentEvent::UserText(text) => Observation::User(text.clone()),
            AgentEvent::ToolCall { title, status, .. } => {
                Observation::Tool(format!("{title} · {}", tool_status(*status)))
            }
            AgentEvent::ToolCallUpdate { title, status, .. } => Observation::Tool(format!(
                "{} · {}",
                title.as_deref().unwrap_or("Command"),
                status.map(tool_status).unwrap_or("Updated")
            )),
            AgentEvent::TurnEnded(end) => Observation::Ended(ended(*end).into()),
            AgentEvent::Notice(text) => Observation::Notice(text.clone()),
            AgentEvent::Error(text) | AgentEvent::PromptRejected { error: text, .. } => {
                Observation::Error(text.clone())
            }
            AgentEvent::PermissionLapsed { title, .. } => {
                Observation::Notice(format!("The request expired and was denied: {title}"))
            }
            AgentEvent::Lagged { .. } => {
                Observation::Notice("Some earlier updates are unavailable.".into())
            }
            _ => return None,
        },
        _ => return None,
    })
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
fn tool_status(status: ToolStatus) -> &'static str {
    match status {
        ToolStatus::Pending => "Waiting",
        ToolStatus::InProgress => "Running",
        ToolStatus::Completed => "Finished",
        ToolStatus::Failed => "Failed",
    }
}
fn ended(end: TurnEnd) -> &'static str {
    match end {
        TurnEnd::EndTurn => "Finished",
        TurnEnd::Cancelled => "Stopped",
        TurnEnd::Refusal => "Declined",
        TurnEnd::MaxTokens => "Token limit reached",
        TurnEnd::MaxTurnRequests => "Request limit reached",
        TurnEnd::Other => "Ended",
    }
}

#[cfg(test)]
#[path = "wire.tests.rs"]
mod tests;
