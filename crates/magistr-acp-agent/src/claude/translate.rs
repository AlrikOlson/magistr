//! CLI frames → ACP session updates, one turn at a time.

use agent_client_protocol::schema::v1::{
    ContentBlock as AcpContent, ContentChunk, Cost, SessionUpdate, StopReason, TextContent,
    ToolCall, ToolCallLocation, ToolCallStatus, ToolCallUpdate, ToolCallUpdateFields, UsageUpdate,
};

use crate::claude::frame::{ContentBlock, Frame, McpServerStatus, ResultFrame, StreamEvent};
use crate::claude::permission::PermissionAsk;
use crate::claude::tool_kind::tool_kind;

/// What one frame means for the turn: updates to emit, an ask to relay, a
/// control request to refuse, or the turn's end.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Step {
    /// Emit this `session/update`.
    Update(Box<SessionUpdate>),
    /// Relay this permission ask to the client and answer the CLI.
    Ask {
        request_id: String,
        ask: PermissionAsk,
    },
    /// A control request the provider does not serve; answer it with an
    /// error so the CLI does not wait.
    Refuse { request_id: String, subtype: String },
    /// The turn ended.
    Done(TurnEnd),
}

/// How a turn ended: a stop reason for the client, or a failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TurnEnd {
    /// The turn ended and the client gets this stop reason.
    Stop(StopReason),
    /// The CLI reported the turn as failed for this reason.
    Failed(String),
}

/// The translator for one session: it carries the little state the mapping
/// needs across frames — the model (for the context window), whether the
/// turn was interrupted, and which blocks were already streamed so a
/// completed `assistant` block is not delivered twice.
#[derive(Debug, Default)]
pub(crate) struct TurnTranslator {
    session_id: Option<String>,
    model: Option<String>,
    mcp_servers: Vec<McpServerStatus>,
    interrupted: bool,
    streamed_text: bool,
    streamed_thinking: bool,
}

impl TurnTranslator {
    /// The context window assumed when the result names none.
    pub const DEFAULT_WINDOW: u64 = 200_000;

    pub fn new() -> Self {
        Self::default()
    }

    /// The CLI's session id, once an `init` frame named it.
    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    /// The model the CLI resolved, once an `init` frame named it.
    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    /// The MCP servers and their statuses from the latest `init` frame.
    pub fn mcp_servers(&self) -> &[McpServerStatus] {
        &self.mcp_servers
    }

    /// The provider sent an interrupt: the turn's result, whatever it says,
    /// ends the turn cancelled.
    pub fn mark_interrupted(&mut self) {
        self.interrupted = true;
    }

    /// Whether the turn is known to be interrupted.
    pub fn is_interrupted(&self) -> bool {
        self.interrupted
    }

    /// Forget the per-turn state before the next turn.
    pub fn next_turn(&mut self) {
        self.interrupted = false;
        self.streamed_text = false;
        self.streamed_thinking = false;
    }

    /// The steps one frame calls for.
    pub fn translate(&mut self, frame: &Frame) -> Vec<Step> {
        match frame {
            Frame::Init(init) => {
                self.session_id = Some(init.session_id.clone());
                self.model = Some(init.model.clone());
                self.mcp_servers.clone_from(&init.mcp_servers);
                Vec::new()
            }
            Frame::Stream(event) => self.stream(event),
            Frame::Assistant(blocks) => blocks
                .iter()
                .filter_map(|b| self.assistant_block(b))
                .collect(),
            Frame::User(blocks) => blocks.iter().filter_map(|b| self.user_block(b)).collect(),
            Frame::Result(result) => self.result(result),
            Frame::ControlRequest(request) => vec![match PermissionAsk::parse(&request.request) {
                Some(ask) if request.subtype == "can_use_tool" => Step::Ask {
                    request_id: request.request_id.clone(),
                    ask,
                },
                _ => Step::Refuse {
                    request_id: request.request_id.clone(),
                    subtype: request.subtype.clone(),
                },
            }],
            Frame::ControlResponse(_) | Frame::Other => Vec::new(),
        }
    }

    fn stream(&mut self, event: &StreamEvent) -> Vec<Step> {
        match event {
            StreamEvent::TextDelta { text, .. } if !text.is_empty() => {
                self.streamed_text = true;
                vec![update(SessionUpdate::AgentMessageChunk(chunk(text)))]
            }
            StreamEvent::ThinkingDelta { thinking, .. } if !thinking.is_empty() => {
                self.streamed_thinking = true;
                vec![update(SessionUpdate::AgentThoughtChunk(chunk(thinking)))]
            }
            _ => Vec::new(),
        }
    }

    /// A completed block: a tool call is announced here, with its full
    /// input as `rawInput` and the file it names as its location; text
    /// and thinking were streamed as deltas and are only delivered
    /// whole when no delta ever arrived for them.
    fn assistant_block(&mut self, block: &ContentBlock) -> Option<Step> {
        match block {
            ContentBlock::ToolUse { id, name, input } => Some(update(SessionUpdate::ToolCall(
                ToolCall::new(id.clone(), title(name, input))
                    .kind(tool_kind(name))
                    .status(ToolCallStatus::Pending)
                    .raw_input(input.clone())
                    .locations(locations(input)),
            ))),
            ContentBlock::Text { text } => {
                let streamed = std::mem::take(&mut self.streamed_text);
                (!streamed && !text.is_empty())
                    .then(|| update(SessionUpdate::AgentMessageChunk(chunk(text))))
            }
            ContentBlock::Thinking { thinking } => {
                let streamed = std::mem::take(&mut self.streamed_thinking);
                (!streamed && !thinking.is_empty())
                    .then(|| update(SessionUpdate::AgentThoughtChunk(chunk(thinking))))
            }
            _ => None,
        }
    }

    /// A user-side block: a tool result closes its call, its content
    /// forwarded twice — as one text block for a client that shows
    /// words, and whole as `rawOutput` for one that keeps the record;
    /// the CLI's own notes (an interrupt) are read, not shown.
    fn user_block(&mut self, block: &ContentBlock) -> Option<Step> {
        match block {
            ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => {
                let text = ContentBlock::result_text(content);
                let mut fields = ToolCallUpdateFields::new().status(if *is_error {
                    ToolCallStatus::Failed
                } else {
                    ToolCallStatus::Completed
                });
                if !text.is_empty() {
                    fields = fields.content(vec![AcpContent::Text(TextContent::new(text)).into()]);
                }
                if !content.is_null() {
                    fields = fields.raw_output(content.clone());
                }
                Some(update(SessionUpdate::ToolCallUpdate(ToolCallUpdate::new(
                    tool_use_id.clone(),
                    fields,
                ))))
            }
            ContentBlock::Text { text } if text.starts_with("[Request interrupted") => {
                self.interrupted = true;
                None
            }
            _ => None,
        }
    }

    fn result(&mut self, result: &ResultFrame) -> Vec<Step> {
        let used = result.usage.input_tokens
            + result.usage.cache_read_input_tokens
            + result.usage.cache_creation_input_tokens
            + result.usage.output_tokens;
        let size = self
            .model
            .as_deref()
            .and_then(|m| result.model_usage.get(m))
            .map(|u| u.context_window)
            .filter(|w| *w > 0)
            .or_else(|| result.model_usage.values().map(|u| u.context_window).max())
            .filter(|w| *w > 0)
            .unwrap_or(Self::DEFAULT_WINDOW);
        let usage = UsageUpdate::new(used, size).cost(Cost::new(result.total_cost_usd, "USD"));
        let end = if self.interrupted {
            TurnEnd::Stop(StopReason::Cancelled)
        } else if result.subtype == "success" {
            TurnEnd::Stop(match result.stop_reason.as_deref() {
                Some("max_tokens") => StopReason::MaxTokens,
                Some("refusal") => StopReason::Refusal,
                _ => StopReason::EndTurn,
            })
        } else if result.subtype == "error_max_turns" {
            TurnEnd::Stop(StopReason::MaxTurnRequests)
        } else {
            let reason = if result.errors.is_empty() {
                if result.result.is_empty() {
                    result.subtype.clone()
                } else {
                    result.result.clone()
                }
            } else {
                result.errors.join("; ")
            };
            TurnEnd::Failed(reason)
        };
        vec![update(SessionUpdate::UsageUpdate(usage)), Step::Done(end)]
    }
}

/// A step that emits `update`.
fn update(update: SessionUpdate) -> Step {
    Step::Update(Box::new(update))
}

fn chunk(text: &str) -> ContentChunk {
    ContentChunk::new(AcpContent::Text(TextContent::new(text.to_owned())))
}

/// The file a call touches, when its input names one the way the CLI's
/// file tools do (`file_path`), with the line a `Read` starts at when
/// it gave an `offset`. Empty for every other call.
fn locations(input: &serde_json::Value) -> Vec<ToolCallLocation> {
    input["file_path"]
        .as_str()
        .map(|path| {
            let line = input["offset"]
                .as_u64()
                .and_then(|offset| u32::try_from(offset).ok());
            ToolCallLocation::new(path).line(line)
        })
        .into_iter()
        .collect()
}

/// The tool call's title: the tool's name, and the CLI's one-line
/// description of the call when the input carries one.
fn title(name: &str, input: &serde_json::Value) -> String {
    match input["description"].as_str() {
        Some(description) if !description.is_empty() => format!("{name}: {description}"),
        _ => name.to_owned(),
    }
}

#[cfg(test)]
#[path = "translate.tests.rs"]
mod tests;
