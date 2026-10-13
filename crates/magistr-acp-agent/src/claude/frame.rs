//! One line of the CLI's stdout, parsed as far as the provider reads it.
//!
//! Parsing is deliberately loose: every field the provider does not use is
//! ignored, an unknown frame is [`Frame::Other`], and a field that is missing
//! or of the wrong type takes its default — the wire is private and grows
//! with every CLI release, and the provider must keep working through
//! additions. What it does read is pinned by the recorded fixtures.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

/// A frame the CLI wrote, by the `type` it carries.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Frame {
    /// `system`/`init`: the session's identity at the top of a turn.
    Init(InitFrame),
    /// `stream_event`: one Messages API streaming event.
    Stream(StreamEvent),
    /// `assistant`: the completed content blocks of one assistant message.
    Assistant(Vec<ContentBlock>),
    /// `user`: a user message — tool results, or the CLI's own notes.
    User(Vec<ContentBlock>),
    /// `result`: the end of a turn.
    Result(ResultFrame),
    /// `control_request`: the CLI asks the provider something.
    ControlRequest(ControlRequestFrame),
    /// `control_response`: the CLI answers a request the provider sent.
    ControlResponse(ControlResponseFrame),
    /// Anything else — status, thinking-token estimates, rate limits, …
    Other,
}

impl Frame {
    /// Parse one stdout line. A line that is not a JSON object is `Other`.
    pub fn parse(line: &str) -> Self {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            return Self::Other;
        };
        Self::from_value(&value)
    }

    /// Parse an already-decoded frame.
    pub fn from_value(value: &Value) -> Self {
        match value["type"].as_str() {
            Some("system") if value["subtype"] == "init" => {
                from_json(value).map_or(Self::Other, Self::Init)
            }
            Some("stream_event") => Self::Stream(StreamEvent::from_event(&value["event"])),
            Some("assistant") => Self::Assistant(blocks(&value["message"]["content"])),
            Some("user") => Self::User(blocks(&value["message"]["content"])),
            Some("result") => from_json(value).map_or(Self::Other, Self::Result),
            Some("control_request") => {
                let request = &value["request"];
                match (value["request_id"].as_str(), request["subtype"].as_str()) {
                    (Some(request_id), Some(subtype)) => {
                        Self::ControlRequest(ControlRequestFrame {
                            request_id: request_id.to_owned(),
                            subtype: subtype.to_owned(),
                            request: request.clone(),
                        })
                    }
                    _ => Self::Other,
                }
            }
            Some("control_response") => {
                let response = &value["response"];
                match response["request_id"].as_str() {
                    Some(request_id) => Self::ControlResponse(ControlResponseFrame {
                        request_id: request_id.to_owned(),
                        error: response["error"].as_str().map(str::to_owned).or_else(|| {
                            (response["subtype"] == "error")
                                .then(|| "control request failed".to_owned())
                        }),
                        payload: response["response"].clone(),
                    }),
                    None => Self::Other,
                }
            }
            _ => Self::Other,
        }
    }
}

fn from_json<T: for<'de> Deserialize<'de>>(value: &Value) -> Option<T> {
    serde_json::from_value(value.clone()).ok()
}

/// The content blocks of a message; a string content is one text block.
fn blocks(content: &Value) -> Vec<ContentBlock> {
    match content {
        Value::String(text) => vec![ContentBlock::Text { text: text.clone() }],
        Value::Array(items) => items
            .iter()
            .map(|item| from_json(item).unwrap_or(ContentBlock::Other))
            .collect(),
        _ => Vec::new(),
    }
}

/// `system`/`init`.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub(crate) struct InitFrame {
    /// The CLI's session id — the one `--resume` takes.
    pub session_id: String,
    /// The model the turn runs on, as the CLI resolved it.
    pub model: String,
    /// Every MCP server the session has, with its connection status.
    pub mcp_servers: Vec<McpServerStatus>,
    /// The permission mode in force.
    #[serde(rename = "permissionMode")]
    pub permission_mode: String,
}

/// One entry of `init.mcp_servers`.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub(crate) struct McpServerStatus {
    /// The server's name from the MCP config.
    pub name: String,
    /// `connected`, `failed`, `pending`, …
    pub status: String,
}

/// The streaming events the provider reads; the rest are [`StreamEvent::Other`].
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum StreamEvent {
    /// `content_block_start`: a block opens at `index`.
    BlockStart { index: u64, block: ContentBlock },
    /// `content_block_delta` with a `text_delta`.
    TextDelta { index: u64, text: String },
    /// `content_block_delta` with a `thinking_delta`.
    ThinkingDelta { index: u64, thinking: String },
    /// `message_delta`: the message's stop reason, once known.
    MessageDelta { stop_reason: Option<String> },
    /// Everything else: `message_start`, `message_stop`, `content_block_stop`,
    /// `input_json_delta`, `signature_delta`, pings.
    Other,
}

impl StreamEvent {
    fn from_event(event: &Value) -> Self {
        let index = event["index"].as_u64().unwrap_or_default();
        match event["type"].as_str() {
            Some("content_block_start") => Self::BlockStart {
                index,
                block: from_json(&event["content_block"]).unwrap_or(ContentBlock::Other),
            },
            Some("content_block_delta") => {
                let delta = &event["delta"];
                match delta["type"].as_str() {
                    Some("text_delta") => Self::TextDelta {
                        index,
                        text: delta["text"].as_str().unwrap_or_default().to_owned(),
                    },
                    Some("thinking_delta") => Self::ThinkingDelta {
                        index,
                        thinking: delta["thinking"].as_str().unwrap_or_default().to_owned(),
                    },
                    _ => Self::Other,
                }
            }
            Some("message_delta") => Self::MessageDelta {
                stop_reason: event["delta"]["stop_reason"].as_str().map(str::to_owned),
            },
            _ => Self::Other,
        }
    }
}

/// A Messages API content block, as far as the provider reads it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum ContentBlock {
    /// Prose.
    Text {
        #[serde(default)]
        text: String,
    },
    /// Reasoning.
    Thinking {
        #[serde(default)]
        thinking: String,
    },
    /// The model calls a tool; `input` is complete only on `assistant` frames.
    ToolUse {
        #[serde(default)]
        id: String,
        #[serde(default)]
        name: String,
        #[serde(default)]
        input: Value,
    },
    /// A tool's output, on `user` frames.
    ToolResult {
        #[serde(default)]
        tool_use_id: String,
        #[serde(default)]
        content: Value,
        #[serde(default)]
        is_error: bool,
    },
    /// Images, documents, redacted thinking, …
    #[serde(other)]
    Other,
}

impl ContentBlock {
    /// The text of a `tool_result`'s content: a string as is, an array of
    /// blocks by their text fields joined with newlines.
    pub fn result_text(content: &Value) -> String {
        match content {
            Value::String(text) => text.clone(),
            Value::Array(items) => items
                .iter()
                .filter_map(|item| item["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n"),
            _ => String::new(),
        }
    }
}

/// `result`: the end of a turn.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub(crate) struct ResultFrame {
    /// `success`, `error_max_turns`, `error_max_budget_usd`, `error_during_execution`, …
    pub subtype: String,
    /// Whether the turn failed (an interrupted turn is reported this way too).
    pub is_error: bool,
    /// The model's final stop reason, `None` when the turn never reached one.
    pub stop_reason: Option<String>,
    /// The final text of the turn.
    pub result: String,
    /// The errors of a failed turn.
    pub errors: Vec<String>,
    /// Main-loop token usage of this turn.
    pub usage: Usage,
    /// Per-model totals, cumulative over the session; the context window lives here.
    #[serde(rename = "modelUsage")]
    pub model_usage: BTreeMap<String, ModelUsage>,
    /// Cumulative estimated cost in USD, when the provider reported it.
    pub total_cost_usd: Option<f64>,
    /// Tool calls the session denied this turn.
    pub permission_denials: Vec<Value>,
}

/// `result.usage`, the fields the context gauge needs.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub(crate) struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_input_tokens: u64,
    pub cache_creation_input_tokens: u64,
}

/// One entry of `result.modelUsage`.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub(crate) struct ModelUsage {
    /// The model's context window, in tokens.
    #[serde(rename = "contextWindow")]
    pub context_window: u64,
    /// The cost attributed to this model, cumulative.
    #[serde(rename = "costUSD")]
    pub cost_usd: f64,
}

/// `control_request` from the CLI.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ControlRequestFrame {
    /// Echoed on the `control_response`.
    pub request_id: String,
    /// `can_use_tool`, `hook_callback`, `mcp_message`, …
    pub subtype: String,
    /// The whole request object, for the subtype's own parser.
    pub request: Value,
}

/// `control_response` from the CLI, answering a request the provider sent.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ControlResponseFrame {
    /// The request this answers.
    pub request_id: String,
    /// `Some` when the CLI refused the request.
    pub error: Option<String>,
    /// The success payload (the initialize response, the interrupt receipt, …).
    pub payload: Value,
}

#[cfg(test)]
#[path = "frame.tests.rs"]
mod tests;
