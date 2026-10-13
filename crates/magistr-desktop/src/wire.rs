//! The desktop's bounded, typed view of application observations and their identity.
use magistr::application::SessionSnapshot;
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
pub(crate) struct Usage {
    used: u64,
    size: u64,
    cost_cents: Option<u64>,
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
        let mut status = snapshot.status.label().to_owned();
        let model = snapshot.model.clone();
        let usage = match snapshot.latest_usage() {
            Some(Msg::Agent(AgentEvent::Usage {
                used,
                size,
                cost_cents,
                ..
            })) => Some(Usage {
                used: *used,
                size: *size,
                cost_cents: *cost_cents,
            }),
            _ => None,
        };
        if snapshot.pending_requests().next().is_some() {
            status = "Waiting for you".into();
        }
        if snapshot.closed {
            status = "Closed".into();
        }
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
