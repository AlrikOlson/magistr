//! The scripted provider.

use agent_client_protocol::BoxFuture;
use agent_client_protocol::schema::v1::{
    ContentBlock, ContentChunk, PermissionOption, PermissionOptionKind, Plan, PlanEntry,
    PlanEntryPriority, PlanEntryStatus, RequestPermissionOutcome, SessionUpdate, StopReason,
    TextContent, ToolCall, ToolCallStatus, ToolCallUpdate, ToolCallUpdateFields, ToolKind,
    UsageUpdate,
};

use crate::{AgentError, ModelChoice, Provider, SessionOpened, SessionStart, Turn};

/// A provider that proves the agent's lifecycle without a model. Every turn
/// thinks, echoes the prompt on the session's model, and reports usage; words
/// in the prompt script the rest:
///
/// | word in the prompt | what the turn does |
/// |---|---|
/// | `permission` | announces a `Bash` tool call, asks the client for permission, then reports the call completed, failed or cancelled |
/// | `plan` | publishes a two-entry plan |
/// | `hang` | says it is hanging and waits for `session/cancel` |
/// | `fail` | fails the turn with a provider error |
///
/// Resuming a session (`session/load`) replays one line naming it. Serves
/// the duplex tests, the end-to-end test through magistr's own client, and
/// `magistr acp-agent --provider scripted` as a smoke agent.
#[derive(Debug, Default, Clone, Copy)]
pub struct ScriptedProvider;

impl ScriptedProvider {
    /// The provider's name on the command line.
    pub const NAME: &'static str = "scripted";
    /// The default model.
    pub const FAST: &'static str = "scripted-fast";
    /// The other model, so a switch is observable in the echo.
    pub const SLOW: &'static str = "scripted-slow";
    /// The context window the usage update reports.
    const WINDOW: u64 = 200_000;
}

impl Provider for ScriptedProvider {
    fn capabilities(&self) -> crate::ProviderCapabilities {
        // This fixture synthesizes the same replay in every fresh process.
        crate::ProviderCapabilities {
            durable_load: true,
            history_replay: true,
            tools: true,
            ..crate::ProviderCapabilities::text_session()
        }
    }
    fn name(&self) -> &str {
        Self::NAME
    }

    fn models(&self) -> Vec<ModelChoice> {
        vec![
            ModelChoice::new(Self::FAST, "Scripted (fast)"),
            ModelChoice::new(Self::SLOW, "Scripted (slow)"),
        ]
    }

    fn start(&self, session: SessionStart) -> BoxFuture<'_, Result<SessionOpened, AgentError>> {
        Box::pin(async move {
            session.validate(self)?;
            let replay = if session.resume {
                vec![SessionUpdate::AgentMessageChunk(ContentChunk::new(
                    ContentBlock::Text(TextContent::new(format!("resumed {}", session.session_id))),
                ))]
            } else {
                Vec::new()
            };
            let mut opened = SessionOpened::new(session.session_id).replay(replay);
            opened.model = session.model.or_else(|| Some(Self::FAST.to_owned()));
            Ok(opened)
        })
    }

    fn prompt(&self, turn: Turn) -> BoxFuture<'_, Result<StopReason, AgentError>> {
        Box::pin(async move { run_turn(turn).await })
    }
}

async fn run_turn(turn: Turn) -> Result<StopReason, AgentError> {
    let text = turn.text();
    let words = text.to_lowercase();
    let model = turn.model().unwrap_or("no-model").to_owned();

    turn.thought(format!("considering: {text}"))?;
    turn.message(format!("[{model}] echo: {text}"))?;

    if words.contains("fail") {
        return Err(AgentError::provider("scripted failure"));
    }
    if words.contains("plan") {
        turn.plan(Plan::new(vec![
            PlanEntry::new(
                "read the prompt",
                PlanEntryPriority::High,
                PlanEntryStatus::Completed,
            ),
            PlanEntry::new(
                "answer it",
                PlanEntryPriority::Medium,
                PlanEntryStatus::InProgress,
            ),
        ]))?;
    }
    if words.contains("permission") {
        let stop = ask_permission(&turn).await?;
        if stop != StopReason::EndTurn {
            return Ok(stop);
        }
    }
    if words.contains("hang") {
        turn.message("hanging until cancel")?;
        turn.cancelled().await;
        return Ok(StopReason::Cancelled);
    }

    turn.usage(UsageUpdate::new(
        text.len() as u64,
        ScriptedProvider::WINDOW,
    ))?;
    Ok(StopReason::EndTurn)
}

/// Announce a `Bash` call, ask, and report the answer on the call.
async fn ask_permission(turn: &Turn) -> Result<StopReason, AgentError> {
    const CALL: &str = "tc-1";
    turn.tool_call(ToolCall::new(CALL, "Bash").kind(ToolKind::Execute))?;
    let outcome = turn
        .request_permission(
            ToolCallUpdate::new(
                CALL,
                ToolCallUpdateFields::new()
                    .title("Bash".to_owned())
                    .raw_input(serde_json::json!({"command":"scripted-action"})),
            ),
            vec![
                PermissionOption::new("allow", "Allow", PermissionOptionKind::AllowOnce),
                PermissionOption::new("reject", "Reject", PermissionOptionKind::RejectOnce),
            ],
        )
        .await?;
    let (status, verdict, stop) = match outcome {
        RequestPermissionOutcome::Selected(selected)
            if selected.option_id.to_string() == "allow" =>
        {
            (ToolCallStatus::Completed, "allowed", StopReason::EndTurn)
        }
        RequestPermissionOutcome::Selected(_) => {
            (ToolCallStatus::Failed, "rejected", StopReason::EndTurn)
        }
        _ => (ToolCallStatus::Failed, "cancelled", StopReason::Cancelled),
    };
    turn.tool_call_update(ToolCallUpdate::new(
        CALL,
        ToolCallUpdateFields::new().status(status),
    ))?;
    turn.message(format!("permission: {verdict}"))?;
    Ok(stop)
}
