//! A scripted in-process ACP client for driving the agent without a
//! subprocess — the mirror of magistr-acp's `fake_agent`: the agent takes one
//! end of a `Channel::duplex()`, the client the other.

// Shared by two test binaries that each use a subset of it.
#![allow(dead_code)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::schema::v1::{
    ClientCapabilities, ContentBlock, InitializeRequest, InitializeResponse, NewSessionRequest,
    NewSessionResponse, PromptRequest, PromptResponse, RequestPermissionOutcome,
    RequestPermissionRequest, RequestPermissionResponse, SelectedPermissionOutcome, SessionId,
    SessionNotification, SessionUpdate, TextContent,
};
use agent_client_protocol::{
    Agent, Channel, Client, ConnectionTo, Error, Responder, on_receive_notification,
    on_receive_request,
};
use magistr_acp_agent::{Provider, ScriptedProvider, serve};

pub const WAIT: Duration = Duration::from_secs(10);

/// How the fake client answers `session/request_permission`.
#[derive(Clone, Copy, Debug, Default)]
pub enum PermissionAnswer {
    /// Select the option with this id at once.
    Select(&'static str),
    /// Answer `cancelled` at once.
    #[default]
    Cancel,
    /// Hold the request open; `Observed::pending_permission` gets the responder.
    Hold,
}

/// What the client saw and holds.
#[derive(Default)]
pub struct Observed {
    /// Every `session/update`, in arrival order, with its session.
    pub updates: Mutex<Vec<(SessionId, SessionUpdate)>>,
    /// Every permission request's tool call title.
    pub permission_requests: Mutex<Vec<String>>,
    /// The responder of a held permission request.
    pub pending_permission: Mutex<Option<Responder<RequestPermissionResponse>>>,
}

impl Observed {
    /// The text of every `agent_message_chunk` so far, concatenated.
    pub fn messages(&self) -> String {
        self.updates
            .lock()
            .expect("lock")
            .iter()
            .filter_map(|(_, u)| match u {
                SessionUpdate::AgentMessageChunk(chunk) => text_of(&chunk.content),
                _ => None,
            })
            .collect()
    }

    /// Whether any update matches `pred`.
    pub fn saw(&self, pred: impl Fn(&SessionUpdate) -> bool) -> bool {
        self.updates
            .lock()
            .expect("lock")
            .iter()
            .any(|(_, u)| pred(u))
    }

    /// Wait until an update matching `pred` has arrived.
    pub async fn wait_for(&self, pred: impl Fn(&SessionUpdate) -> bool) {
        tokio::time::timeout(WAIT, async {
            while !self.saw(&pred) {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("the update arrived before the timeout");
    }

    /// Answer the held permission request.
    pub fn answer_pending(&self, outcome: RequestPermissionOutcome) {
        let responder = self
            .pending_permission
            .lock()
            .expect("lock")
            .take()
            .expect("a held permission request");
        responder
            .respond(RequestPermissionResponse::new(outcome))
            .expect("answer the permission request");
    }
}

fn text_of(block: &ContentBlock) -> Option<String> {
    match block {
        ContentBlock::Text(t) => Some(t.text.clone()),
        _ => None,
    }
}

/// Run the scripted agent on one end of a duplex and `body` as the client on
/// the other. Returns what `body` returned and what the client observed.
pub async fn with_client<T: Send + 'static>(
    answer: PermissionAnswer,
    body: impl AsyncFnOnce(ConnectionTo<Agent>, Arc<Observed>) -> Result<T, Error>,
) -> (T, Arc<Observed>) {
    with_provider(ScriptedProvider, answer, body).await
}

/// [`with_client`] on any provider.
pub async fn with_provider<T: Send + 'static>(
    provider: impl Provider,
    answer: PermissionAnswer,
    body: impl AsyncFnOnce(ConnectionTo<Agent>, Arc<Observed>) -> Result<T, Error>,
) -> (T, Arc<Observed>) {
    let (client_end, agent_end) = Channel::duplex();
    let _agent = tokio::spawn(serve(provider, agent_end));
    let observed = Arc::new(Observed::default());

    let outcome = Client
        .builder()
        .name("fake-client")
        .on_receive_notification(
            {
                let observed = Arc::clone(&observed);
                async move |n: SessionNotification, _cx| {
                    observed
                        .updates
                        .lock()
                        .expect("lock")
                        .push((n.session_id, n.update));
                    Ok(())
                }
            },
            on_receive_notification!(),
        )
        .on_receive_request(
            {
                let observed = Arc::clone(&observed);
                async move |req: RequestPermissionRequest, responder, _cx| {
                    observed
                        .permission_requests
                        .lock()
                        .expect("lock")
                        .push(req.tool_call.fields.title.clone().unwrap_or_default());
                    match answer {
                        PermissionAnswer::Select(id) => responder.respond(
                            RequestPermissionResponse::new(RequestPermissionOutcome::Selected(
                                SelectedPermissionOutcome::new(id),
                            )),
                        ),
                        PermissionAnswer::Cancel => responder.respond(
                            RequestPermissionResponse::new(RequestPermissionOutcome::Cancelled),
                        ),
                        PermissionAnswer::Hold => {
                            *observed.pending_permission.lock().expect("lock") = Some(responder);
                            Ok(())
                        }
                    }
                }
            },
            on_receive_request!(),
        )
        .connect_with(client_end, {
            let observed = Arc::clone(&observed);
            async move |cx| {
                tokio::time::timeout(WAIT, body(cx, Arc::clone(&observed)))
                    .await
                    .expect("the client body finished before the timeout")
            }
        })
        .await
        .expect("the client connection ran to completion");

    (outcome, observed)
}

/// `initialize` on v1.
pub async fn initialize(cx: &ConnectionTo<Agent>) -> Result<InitializeResponse, Error> {
    cx.send_request(
        InitializeRequest::new(ProtocolVersion::V1).client_capabilities(ClientCapabilities::new()),
    )
    .block_task()
    .await
}

/// `session/new` in a scratch directory.
pub async fn new_session(cx: &ConnectionTo<Agent>) -> Result<NewSessionResponse, Error> {
    cx.send_request(NewSessionRequest::new(std::env::temp_dir()))
        .block_task()
        .await
}

/// A text prompt.
pub fn prompt(session_id: &SessionId, text: &str) -> PromptRequest {
    PromptRequest::new(
        session_id.clone(),
        vec![ContentBlock::Text(TextContent::new(text))],
    )
}

/// Send a text prompt and wait for the turn to end.
pub async fn prompt_and_wait(
    cx: &ConnectionTo<Agent>,
    session_id: &SessionId,
    text: &str,
) -> Result<PromptResponse, Error> {
    cx.send_request(prompt(session_id, text)).block_task().await
}
