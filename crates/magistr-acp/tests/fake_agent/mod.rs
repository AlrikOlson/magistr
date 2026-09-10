//! A scripted in-process ACP agent for driving the transport without a
//! subprocess — the SDK's own testing pattern: one end of a
//! `Channel::duplex()` is the agent, the other is handed to
//! `AcpTransport::connect_over`.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::schema::v1::{
    AgentCapabilities, CancelNotification, ClientCapabilities, ContentBlock, ContentChunk,
    CreateTerminalRequest, InitializeRequest, InitializeResponse, ListSessionsRequest,
    ListSessionsResponse, LoadSessionRequest, LoadSessionResponse, Meta, NewSessionRequest,
    NewSessionResponse, PermissionOption, PermissionOptionKind, PromptRequest, PromptResponse,
    ReadTextFileRequest, ReleaseTerminalRequest, RequestPermissionOutcome,
    RequestPermissionRequest, SessionConfigOption, SessionConfigOptionCategory,
    SessionConfigOptionValue, SessionConfigSelectOption, SessionId, SessionInfo,
    SessionNotification, SessionUpdate, SetSessionConfigOptionRequest,
    SetSessionConfigOptionResponse, StopReason, TerminalExitStatus, TerminalOutputRequest,
    TextContent, ToolCallUpdate, ToolCallUpdateFields, WaitForTerminalExitRequest,
    WriteTextFileRequest,
};
use agent_client_protocol::{
    Agent, Channel, Error, JsonRpcRequest, JsonRpcResponse, Responder, on_receive_notification,
    on_receive_request,
};
use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;

pub const SESSION_ID: &str = "sess-1";
pub const FABLE: &str = "claude-fable-5[1m]";

/// What the fake agent does during a prompt.
#[derive(Default, Clone)]
pub struct Script {
    /// Advertise `loadSession` and accept `session/load`.
    pub supports_load: bool,
    /// Ask for permission for a tool call before finishing the turn.
    pub ask_permission: bool,
    /// Read this file through the client before finishing the turn.
    pub read_file: Option<PathBuf>,
    /// Write a file directly through the client.
    pub write_file: Option<(PathBuf, String)>,
    /// Run this command in a client terminal before finishing the turn.
    pub run_terminal: Option<(String, Vec<String>)>,
    /// Never finish the turn on its own — only a `session/cancel` ends it.
    pub hang_until_cancel: bool,
    /// Advertise the `_session/goal` extension in `initialize._meta` and
    /// serve it: a `set` echoes the objective back as a
    /// `session_info_update` with `_meta.goal`, then answers `{}`.
    pub goal_extension: bool,
}

/// The advertisement the fake puts in `initialize._meta` when it serves
/// the goal extension — the shape the retired npm shim (0.70.0) sent.
pub fn goal_meta() -> Meta {
    let mut meta = Meta::new();
    meta.insert(
        "goal".to_owned(),
        serde_json::json!({"version": 1, "controlMethod": "_session/goal", "actions": ["set", "clear"]}),
    );
    meta
}

/// `_session/goal` as the fake agent receives it.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "_session/goal", response = GoalResponse)]
#[serde(rename_all = "camelCase")]
pub struct GoalRequest {
    pub session_id: SessionId,
    pub action: String,
    #[serde(default)]
    pub objective: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
pub struct GoalResponse {}

/// What the fake agent saw the client do.
#[derive(Default)]
pub struct Observed {
    pub client_capabilities: Mutex<Option<ClientCapabilities>>,
    pub effect_errors: Mutex<Vec<Error>>,
    /// The `_meta` the client attached to `session/new` or `session/load`.
    pub session_meta: Mutex<Option<Meta>>,
    pub permission_outcome: Mutex<Option<RequestPermissionOutcome>>,
    pub file_content: Mutex<Option<String>>,
    pub terminal_exit: Mutex<Option<TerminalExitStatus>>,
    pub terminal_output: Mutex<Option<String>>,
    pub loaded_session: Mutex<Option<String>>,
    pub config_sets: Mutex<Vec<(String, String)>>,
    pub cancels: AtomicUsize,
    /// Every `_session/goal` call, as `(action, objective)`.
    pub goals: Mutex<Vec<(String, Option<String>)>>,
}

fn model_options(current: &str) -> Vec<SessionConfigOption> {
    vec![
        SessionConfigOption::select(
            "model",
            "Model",
            current.to_string(),
            vec![
                SessionConfigSelectOption::new("sonnet", "Sonnet"),
                SessionConfigSelectOption::new(FABLE, "Fable"),
            ],
        )
        .category(SessionConfigOptionCategory::Model),
    ]
}

type PendingPrompt = Arc<Mutex<Option<Responder<PromptResponse>>>>;

/// Start the agent on one end of a duplex channel. Returns the client's end,
/// the observations, and the agent task.
pub fn spawn(script: Script) -> (Channel, Arc<Observed>, JoinHandle<Result<(), Error>>) {
    let (client_end, agent_end) = Channel::duplex();
    let observed = Arc::new(Observed::default());
    let pending: PendingPrompt = Arc::new(Mutex::new(None));

    let task = tokio::spawn({
        let observed = Arc::clone(&observed);
        let supports_load = script.supports_load;
        let goal_extension = script.goal_extension;
        Agent
            .builder()
            .name("fake-agent")
            .on_receive_request(
                {
                    let observed = Arc::clone(&observed);
                    async move |req: InitializeRequest, responder, _cx| {
                        *observed.client_capabilities.lock().expect("lock") =
                            Some(req.client_capabilities);
                        let mut response = InitializeResponse::new(ProtocolVersion::V1)
                            .agent_capabilities(
                                AgentCapabilities::new().load_session(supports_load),
                            );
                        if goal_extension {
                            response = response.meta(goal_meta());
                        }
                        responder.respond(response)
                    }
                },
                on_receive_request!(),
            )
            .on_receive_request(
                {
                    let observed = Arc::clone(&observed);
                    async move |req: GoalRequest, responder, cx| {
                        observed
                            .goals
                            .lock()
                            .expect("lock")
                            .push((req.action.clone(), req.objective.clone()));
                        if req.action == "set" {
                            let mut meta = Meta::new();
                            meta.insert(
                                "goal".to_owned(),
                                serde_json::json!({
                                    "objective": req.objective,
                                    "status": "active",
                                    "controlMethod": "_session/goal",
                                }),
                            );
                            cx.send_notification(SessionNotification::new(
                                req.session_id,
                                SessionUpdate::SessionInfoUpdate(
                                    agent_client_protocol::schema::v1::SessionInfoUpdate::new()
                                        .meta(meta),
                                ),
                            ))?;
                        }
                        responder.respond(GoalResponse {})
                    }
                },
                on_receive_request!(),
            )
            .on_receive_request(
                {
                    let observed = Arc::clone(&observed);
                    async move |req: NewSessionRequest, responder, _cx| {
                        *observed.session_meta.lock().expect("lock") = req.meta.clone();
                        responder.respond(
                            NewSessionResponse::new(SESSION_ID)
                                .config_options(model_options("sonnet")),
                        )
                    }
                },
                on_receive_request!(),
            )
            .on_receive_request(
                {
                    let observed = Arc::clone(&observed);
                    async move |req: LoadSessionRequest, responder, _cx| {
                        *observed.loaded_session.lock().expect("lock") =
                            Some(req.session_id.to_string());
                        *observed.session_meta.lock().expect("lock") = req.meta.clone();
                        responder.respond(
                            LoadSessionResponse::new().config_options(model_options("sonnet")),
                        )
                    }
                },
                on_receive_request!(),
            )
            .on_receive_request(
                async move |_req: ListSessionsRequest, responder, _cx| {
                    responder.respond(ListSessionsResponse::new(vec![SessionInfo::new(
                        SESSION_ID, "/tmp",
                    )]))
                },
                on_receive_request!(),
            )
            .on_receive_request(
                {
                    let observed = Arc::clone(&observed);
                    async move |req: SetSessionConfigOptionRequest, responder, _cx| {
                        let value = match &req.value {
                            SessionConfigOptionValue::ValueId { value } => value.to_string(),
                            _ => "?".to_string(),
                        };
                        observed
                            .config_sets
                            .lock()
                            .expect("lock")
                            .push((req.config_id.to_string(), value.clone()));
                        responder
                            .respond(SetSessionConfigOptionResponse::new(model_options(&value)))
                    }
                },
                on_receive_request!(),
            )
            .on_receive_request(
                {
                    let (observed, pending, script) =
                        (Arc::clone(&observed), Arc::clone(&pending), script.clone());
                    async move |req: PromptRequest, responder, cx| {
                        let (observed, pending, script) =
                            (Arc::clone(&observed), Arc::clone(&pending), script.clone());
                        let cx2 = cx.clone();
                        cx.spawn(async move {
                            run_turn(cx2, req.session_id, responder, script, observed, pending)
                                .await
                        })?;
                        Ok(())
                    }
                },
                on_receive_request!(),
            )
            .on_receive_notification(
                {
                    let (observed, pending) = (Arc::clone(&observed), Arc::clone(&pending));
                    async move |_n: CancelNotification, _cx| {
                        observed.cancels.fetch_add(1, Ordering::SeqCst);
                        if let Some(responder) = pending.lock().expect("lock").take() {
                            responder.respond(PromptResponse::new(StopReason::Cancelled))?;
                        }
                        Ok(())
                    }
                },
                on_receive_notification!(),
            )
            .connect_to(agent_end)
    });

    (client_end, observed, task)
}

/// One scripted turn, run off the dispatch loop so it can wait on the client.
async fn run_turn(
    cx: agent_client_protocol::ConnectionTo<agent_client_protocol::Client>,
    session_id: SessionId,
    responder: Responder<PromptResponse>,
    script: Script,
    observed: Arc<Observed>,
    pending: PendingPrompt,
) -> Result<(), Error> {
    cx.send_notification(SessionNotification::new(
        session_id.clone(),
        SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Text(TextContent::new(
            "Hello from the fake agent",
        )))),
    ))?;

    if script.ask_permission {
        let response = cx
            .send_request(RequestPermissionRequest::new(
                session_id.clone(),
                ToolCallUpdate::new(
                    "tc-1",
                    ToolCallUpdateFields::new().title("Bash".to_string()),
                ),
                vec![
                    PermissionOption::new("allow", "Allow", PermissionOptionKind::AllowOnce),
                    PermissionOption::new("reject", "Reject", PermissionOptionKind::RejectOnce),
                ],
            ))
            .block_task()
            .await?;
        *observed.permission_outcome.lock().expect("lock") = Some(response.outcome);
    }

    if let Some(path) = &script.read_file {
        match cx
            .send_request(ReadTextFileRequest::new(session_id.clone(), path.clone()))
            .block_task()
            .await
        {
            Ok(response) => *observed.file_content.lock().expect("lock") = Some(response.content),
            Err(error) => observed.effect_errors.lock().expect("lock").push(error),
        }
    }

    if let Some((path, content)) = &script.write_file
        && let Err(error) = cx
            .send_request(WriteTextFileRequest::new(
                session_id.clone(),
                path.clone(),
                content.clone(),
            ))
            .block_task()
            .await
    {
        observed.effect_errors.lock().expect("lock").push(error);
    }

    if let Some((command, args)) = &script.run_terminal {
        let created = match cx
            .send_request(
                CreateTerminalRequest::new(session_id.clone(), command.clone()).args(args.clone()),
            )
            .block_task()
            .await
        {
            Ok(created) => created,
            Err(error) => {
                observed.effect_errors.lock().expect("lock").push(error);
                return responder.respond(PromptResponse::new(StopReason::EndTurn));
            }
        };
        let tid = created.terminal_id;
        let waited = cx
            .send_request(WaitForTerminalExitRequest::new(
                session_id.clone(),
                tid.clone(),
            ))
            .block_task()
            .await?;
        *observed.terminal_exit.lock().expect("lock") = Some(waited.exit_status);
        let output = cx
            .send_request(TerminalOutputRequest::new(session_id.clone(), tid.clone()))
            .block_task()
            .await?;
        *observed.terminal_output.lock().expect("lock") = Some(output.output);
        cx.send_request(ReleaseTerminalRequest::new(session_id.clone(), tid))
            .block_task()
            .await?;
    }

    if script.hang_until_cancel {
        *pending.lock().expect("lock") = Some(responder);
        return Ok(());
    }
    responder.respond(PromptResponse::new(StopReason::EndTurn))
}
