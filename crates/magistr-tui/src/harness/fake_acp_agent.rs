//! An in-process ACP agent for the session: the transport's other end
//! without a subprocess.

use std::sync::{Arc, Mutex};

use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::schema::v1::{
    AgentCapabilities, AvailableCommand, AvailableCommandsUpdate, CancelNotification, ContentBlock,
    ContentChunk, InitializeRequest, InitializeResponse, Meta, NewSessionRequest,
    NewSessionResponse, PermissionOption, PermissionOptionKind, PromptRequest, PromptResponse,
    RequestPermissionOutcome, RequestPermissionRequest, SessionConfigOption,
    SessionConfigOptionCategory, SessionConfigOptionValue, SessionConfigSelectOption, SessionId,
    SessionInfoUpdate, SessionNotification, SessionUpdate, SetSessionConfigOptionRequest,
    SetSessionConfigOptionResponse, StopReason, TextContent, ToolCallUpdate, ToolCallUpdateFields,
    ToolKind, UsageUpdate,
};
use agent_client_protocol::{
    Agent, Channel, Client, ConnectionTo, Error, JsonRpcRequest, JsonRpcResponse, Responder,
    on_receive_notification, on_receive_request,
};
use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;

use super::acp_observed::AcpObserved;

/// The session id the fake agent hands out.
pub const SESSION_ID: &str = "fake-session";

/// A scripted ACP agent on one end of a [`Channel::duplex`]: it completes
/// the handshake (advertising a model option), and on every prompt says
/// `reply`, asks permission for `permission` when set (offering allow,
/// allow-always and reject), and ends the turn. `session/cancel` ends a
/// turn that is waiting on a permission — or a [`stalling`](Self::stalling)
/// turn that never answers — with `Cancelled`. What the client
/// did is in the [`AcpObserved`] handed back by [`FakeAcpAgent::spawn`].
#[derive(Clone, Debug, PartialEq)]
pub struct FakeAcpAgent {
    reply: String,
    permission: Option<String>,
    kind: Option<ToolKind>,
    input: Option<serde_json::Value>,
    stalling: bool,
    goal_extension: bool,
    goal_command: bool,
    goal_mute: bool,
    usage: Option<(u64, u64)>,
}

impl Default for FakeAcpAgent {
    fn default() -> Self {
        Self {
            reply: "Hello from the fake agent".to_owned(),
            permission: None,
            kind: None,
            input: None,
            stalling: false,
            goal_extension: false,
            goal_command: false,
            goal_mute: false,
            usage: None,
        }
    }
}

/// The `_session/goal` request as the fake serves it — the vendor
/// extension the retired npm adapter advertised in `initialize._meta.goal`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "_session/goal", response = GoalExtensionResponse)]
#[serde(rename_all = "camelCase")]
struct GoalExtensionRequest {
    session_id: SessionId,
    action: String,
    #[serde(default)]
    objective: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
struct GoalExtensionResponse {}

/// The advertisement the fake puts in `initialize._meta` when it serves
/// the goal extension — the shape the real agent sends.
fn goal_advertisement() -> Meta {
    let mut meta = Meta::new();
    meta.insert(
        "goal".to_owned(),
        serde_json::json!({"version": 1, "controlMethod": "_session/goal", "actions": ["set", "clear"]}),
    );
    meta
}

/// The optimistic echo: `session_info_update` with `_meta.goal`.
fn goal_echo(objective: &str) -> SessionUpdate {
    let mut meta = Meta::new();
    meta.insert(
        "goal".to_owned(),
        serde_json::json!({
            "objective": objective,
            "status": "active",
            "controlMethod": "_session/goal",
        }),
    );
    SessionUpdate::SessionInfoUpdate(SessionInfoUpdate::new().meta(meta))
}

impl FakeAcpAgent {
    /// An agent that greets and ends the turn.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// What every turn says.
    #[must_use]
    pub fn reply(mut self, text: &str) -> Self {
        self.reply = text.to_owned();
        self
    }

    /// Ask permission to run the tool `title` on every turn.
    #[must_use]
    pub fn asking_permission(mut self, title: &str) -> Self {
        self.permission = Some(title.to_owned());
        self
    }

    /// Never answer a prompt: the turn's responder is parked until a
    /// `session/cancel` ends it with `Cancelled` — what a judge that
    /// times out looks like from the transport.
    #[must_use]
    pub const fn stalling(mut self) -> Self {
        self.stalling = true;
        self
    }

    /// Advertise the `_session/goal` extension in `initialize._meta`
    /// and serve it: a `set` echoes the objective back as a
    /// `session_info_update`, streams one working line, and answers
    /// `{}`; a `clear` just answers.
    #[must_use]
    pub const fn advertising_goal_extension(mut self) -> Self {
        self.goal_extension = true;
        self
    }

    /// List a `goal` command in `available_commands_update` (sent right
    /// after `session/new`) and parse `/goal …` off a turn's text: the
    /// echo, the reply, end of turn.
    #[must_use]
    pub const fn advertising_goal_command(mut self) -> Self {
        self.goal_command = true;
        self
    }

    /// Keep the advertisements but never send the optimistic echo —
    /// what a lying advertisement, a moved shape or a wrong grammar all
    /// look like from the client.
    #[must_use]
    pub const fn mute_goal(mut self) -> Self {
        self.goal_mute = true;
        self
    }

    /// Report the context window on every turn — a `usage_update` of
    /// `used` tokens in a window of `size`, ahead of the reply.
    #[must_use]
    pub const fn reporting_usage(mut self, used: u64, size: u64) -> Self {
        self.usage = Some((used, size));
        self
    }

    /// Ask the permission as an edit — the cockpit shows it as a diff
    /// approval.
    #[must_use]
    pub const fn as_edit(self) -> Self {
        self.with_kind(ToolKind::Edit)
    }

    /// Ask the permission with this tool kind — what a policy
    /// classifies by.
    #[must_use]
    pub const fn with_kind(mut self, kind: ToolKind) -> Self {
        self.kind = Some(kind);
        self
    }

    /// Ask the permission with this raw tool input — the file path or
    /// command a policy inspects.
    #[must_use]
    pub fn with_input(mut self, input: serde_json::Value) -> Self {
        self.input = Some(input);
        self
    }

    /// Start the agent; the client's end of the channel goes to the CLI
    /// host's `AcpTransport::connect_over`.
    #[must_use]
    pub fn spawn(self) -> (Channel, Arc<AcpObserved>, JoinHandle<Result<(), Error>>) {
        let (client_end, agent_end) = Channel::duplex();
        let observed = Arc::new(AcpObserved::default());
        let pending: Pending = Arc::new(Mutex::new(None));
        let task = tokio::spawn({
            let observed = Arc::clone(&observed);
            let script = self;
            Agent
                .builder()
                .name("fake-acp-agent")
                .on_receive_request(
                    {
                        let goal_extension = script.goal_extension;
                        async move |_req: InitializeRequest, responder, _cx| {
                            let mut response = InitializeResponse::new(ProtocolVersion::V1)
                                .agent_capabilities(AgentCapabilities::new());
                            if goal_extension {
                                response = response.meta(goal_advertisement());
                            }
                            responder.respond(response)
                        }
                    },
                    on_receive_request!(),
                )
                .on_receive_request(
                    {
                        let goal_command = script.goal_command;
                        async move |_req: NewSessionRequest, responder, cx| {
                            if goal_command {
                                cx.send_notification(SessionNotification::new(
                                    SessionId::new(SESSION_ID),
                                    SessionUpdate::AvailableCommandsUpdate(
                                        AvailableCommandsUpdate::new(vec![
                                            AvailableCommand::new(
                                                "compact",
                                                "Clear conversation history",
                                            ),
                                            AvailableCommand::new(
                                                "goal",
                                                "Set a goal — keep working until the condition is met",
                                            ),
                                        ]),
                                    ),
                                ))?;
                            }
                            responder.respond(
                                NewSessionResponse::new(SESSION_ID)
                                    .config_options(model_options("default")),
                            )
                        }
                    },
                    on_receive_request!(),
                )
                .on_receive_request(
                    {
                        let (observed, script) = (Arc::clone(&observed), script.clone());
                        async move |req: GoalExtensionRequest, responder, cx| {
                            observed.saw_goal(req.action.clone(), req.objective.clone());
                            if req.action == "set" && !script.goal_mute {
                                let objective = req.objective.clone().unwrap_or_default();
                                // The words first, the echo last, and the
                                // order is load-bearing. The echo is what
                                // the driver treats as delivery confirmed,
                                // so it stops waiting the moment it lands
                                // and the iteration can run to its end. A
                                // chunk sent *after* the echo is racing the
                                // whole rest of the loop to reach the model
                                // — a race it usually wins and, under
                                // compilation contention, sometimes loses.
                                // Sent before it, the inbox's FIFO order
                                // (the same guarantee the harness's quit key
                                // rests on) puts it on the stage ahead of
                                // everything the driver posts afterwards.
                                //
                                // Nothing about the wire contract requires
                                // either order, and production is unaffected
                                // either way: the cockpit outlives the loop
                                // and folds a late chunk whenever it comes.
                                cx.send_notification(SessionNotification::new(
                                    req.session_id.clone(),
                                    SessionUpdate::AgentMessageChunk(ContentChunk::new(
                                        ContentBlock::Text(TextContent::new(format!(
                                            "working toward: {objective}"
                                        ))),
                                    )),
                                ))?;
                                cx.send_notification(SessionNotification::new(
                                    req.session_id.clone(),
                                    goal_echo(&objective),
                                ))?;
                            }
                            responder.respond(GoalExtensionResponse {})
                        }
                    },
                    on_receive_request!(),
                )
                .on_receive_request(
                    {
                        let observed = Arc::clone(&observed);
                        async move |req: SetSessionConfigOptionRequest, responder, _cx| {
                            let value = match &req.value {
                                SessionConfigOptionValue::ValueId { value } => value.to_string(),
                                _ => "?".to_owned(),
                            };
                            observed.saw_config_set(req.config_id.to_string(), value.clone());
                            responder
                                .respond(SetSessionConfigOptionResponse::new(model_options(&value)))
                        }
                    },
                    on_receive_request!(),
                )
                .on_receive_request(
                    {
                        let (observed, pending) = (Arc::clone(&observed), Arc::clone(&pending));
                        async move |req: PromptRequest, responder, cx| {
                            let (observed, pending, script) =
                                (Arc::clone(&observed), Arc::clone(&pending), script.clone());
                            let turn = cx.clone();
                            let text = prompt_text(&req.prompt);
                            observed.saw_prompt(text.clone());
                            cx.spawn(async move {
                                script
                                    .turn(turn, req.session_id, text, responder, observed, pending)
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
                            observed.saw_cancel();
                            if let Some(responder) = take(&pending) {
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

    /// One turn, off the dispatch loop so it can wait on the client.
    async fn turn(
        self,
        cx: ConnectionTo<Client>,
        session_id: SessionId,
        text: String,
        responder: Responder<PromptResponse>,
        observed: Arc<AcpObserved>,
        pending: Pending,
    ) -> Result<(), Error> {
        if self.goal_command
            && let Some(arg) = text.strip_prefix("/goal")
        {
            let arg = arg.trim();
            if arg == "clear" {
                observed.saw_goal("clear".to_owned(), None);
            } else {
                observed.saw_goal("set".to_owned(), Some(arg.to_owned()));
                if !self.goal_mute {
                    // The reply below is sent AFTER this echo, so it races
                    // the rest of the iteration to reach the model — see
                    // the extension handler above, where the same ordering
                    // was the cause of a real flake. Left as it is only
                    // because no test asserts the command mode's reply is
                    // on the stage: a test that wants to must send its
                    // words before the echo, as the extension now does.
                    cx.send_notification(SessionNotification::new(
                        session_id.clone(),
                        goal_echo(arg),
                    ))?;
                }
            }
            cx.send_notification(SessionNotification::new(
                session_id,
                SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Text(
                    TextContent::new(self.reply.clone()),
                ))),
            ))?;
            return responder.respond(PromptResponse::new(StopReason::EndTurn));
        }
        if self.stalling {
            *pending
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(responder);
            return Ok(());
        }
        if let Some((used, size)) = self.usage {
            cx.send_notification(SessionNotification::new(
                session_id.clone(),
                SessionUpdate::UsageUpdate(UsageUpdate::new(used, size)),
            ))?;
        }
        cx.send_notification(SessionNotification::new(
            session_id.clone(),
            SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Text(
                TextContent::new(self.reply.clone()),
            ))),
        ))?;
        if let Some(title) = &self.permission {
            *pending
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(responder);
            let response = cx
                .send_request(RequestPermissionRequest::new(
                    session_id.clone(),
                    ToolCallUpdate::new("tc-1", {
                        let mut fields = ToolCallUpdateFields::new().title(title.clone());
                        if let Some(kind) = self.kind {
                            fields = fields.kind(kind);
                        }
                        if let Some(input) = self.input.clone() {
                            fields = fields.raw_input(input);
                        }
                        fields
                    }),
                    vec![
                        PermissionOption::new("allow", "Allow", PermissionOptionKind::AllowOnce),
                        PermissionOption::new(
                            "allow_always",
                            "Always",
                            PermissionOptionKind::AllowAlways,
                        ),
                        PermissionOption::new("reject", "Deny", PermissionOptionKind::RejectOnce),
                    ],
                ))
                .block_task()
                .await?;
            observed.saw_outcome(match response.outcome {
                RequestPermissionOutcome::Selected(selected) => {
                    Some(selected.option_id.to_string())
                }
                _ => None,
            });
            // A cancel may have answered the turn meanwhile.
            let Some(responder) = take(&pending) else {
                return Ok(());
            };
            return responder.respond(PromptResponse::new(StopReason::EndTurn));
        }
        responder.respond(PromptResponse::new(StopReason::EndTurn))
    }
}

/// The responder of a turn waiting on the client, so a cancel can end it.
type Pending = Arc<Mutex<Option<Responder<PromptResponse>>>>;

fn take(pending: &Pending) -> Option<Responder<PromptResponse>> {
    pending
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take()
}

fn model_options(current: &str) -> Vec<SessionConfigOption> {
    vec![
        SessionConfigOption::select(
            "model",
            "Model",
            current.to_owned(),
            vec![
                SessionConfigSelectOption::new("default", "Default"),
                SessionConfigSelectOption::new("fast", "Fast"),
            ],
        )
        .category(SessionConfigOptionCategory::Model),
    ]
}

/// The text blocks of a prompt, joined.
fn prompt_text(blocks: &[ContentBlock]) -> String {
    blocks
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .collect()
}
