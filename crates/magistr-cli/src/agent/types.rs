//! Conversion layer between ACP (Agent Client Protocol) types and magistr's `AgentMessage`.
//!
//! The `agent-client-protocol` crate defines the standard message types for
//! editor↔agent communication. This module maps those types to magistr's internal
//! `AgentMessage` enum so the interface and handler code remain backend-agnostic.
//!
//! ## Mapping Summary
//!
//! | ACP Type | magistr Type |
//! |----------|----------|
//! | `SessionUpdate::AgentMessageChunk` | `AgentMessage::Assistant` |
//! | `SessionUpdate::UserMessageChunk` | `AgentMessage::User` |
//! | `SessionUpdate::ToolCall` | `AgentMessage::Assistant` (with `ContentBlock::ToolUse`) |
//! | `SessionUpdate::ToolCallUpdate` | `AgentMessage::StreamEvent` |
//! | `SessionUpdate::Plan` | `AgentMessage::Plan` |
//! | `SessionUpdate::AgentThoughtChunk` | `AgentMessage::Assistant` (with `ContentBlock::Thinking`) |
//! | `SessionUpdate::ConfigOptionUpdate` | `AgentMessage::ConfigUpdate` |
//! | `StopReason::Cancelled` | `AgentMessage::Cancelled` |
//! | `PromptResponse` | `AgentMessage::Result` |
//! | ACP `Error` | `MagistrError::Acp` |

use agent_client_protocol::schema::v1::{
    AvailableCommandInput, ContentBlock as AcpContentBlock, Error as AcpError, Plan,
    PromptResponse, SessionConfigKind, SessionConfigOption, SessionConfigOptionCategory,
    SessionConfigSelectOptions, SessionUpdate, StopReason, ToolCall as AcpToolCall,
    ToolCallContent, ToolCallStatus, ToolCallUpdate, ToolKind,
};

use super::message::{
    AcpConfigOption, AcpPlanEntry, AgentCommand, AgentMessage, AssistantContent,
    ConfigOptionCategory, ConfigSelectValue, ContentBlock, PlanEntryPriority, PlanEntryStatus,
    SessionId, ToolLocation, ToolUseKind, UserBody, UserContent,
};
use crate::error::MagistrError;

// ─── ContentBlock conversions ───────────────────────────────────────────────

/// Convert an ACP `ContentBlock` to magistr's `ContentBlock`.
///
/// Text content is mapped directly. Images are preserved as `ContentBlock::Image`
/// for protocol-aware rendering in the stream panel. Other ACP content types
/// (Audio, Resource, ResourceLink) are converted to descriptive text placeholders.
pub fn content_block_from_acp(block: &AcpContentBlock) -> ContentBlock {
    match block {
        AcpContentBlock::Text(tc) => ContentBlock::Text {
            text: tc.text.clone(),
        },
        AcpContentBlock::Image(img) => {
            let alt = image_alt_text(img);
            ContentBlock::Image {
                mime_type: img.mime_type.clone(),
                uri: img.uri.clone(),
                alt,
            }
        }
        AcpContentBlock::Audio(audio) => ContentBlock::Text {
            text: format!("[audio: {}]", audio.mime_type),
        },
        AcpContentBlock::Resource(res) => ContentBlock::Text {
            text: format_resource_placeholder(res),
        },
        AcpContentBlock::ResourceLink(link) => ContentBlock::Text {
            text: format_resource_link_placeholder(link),
        },
        // ACP ContentBlock is non_exhaustive
        _ => ContentBlock::Text {
            text: "[unsupported content block]".to_string(),
        },
    }
}

/// Extract alt text for an image content block.
///
/// Uses the URI filename if available, otherwise falls back to the MIME type.
fn image_alt_text(img: &agent_client_protocol::schema::v1::ImageContent) -> String {
    match &img.uri {
        Some(uri) => uri.rsplit('/').next().unwrap_or(uri).to_string(),
        None => img.mime_type.clone(),
    }
}

/// Format an embedded resource content block as a descriptive placeholder.
fn format_resource_placeholder(
    res: &agent_client_protocol::schema::v1::EmbeddedResource,
) -> String {
    use agent_client_protocol::schema::v1::EmbeddedResourceResource;
    match &res.resource {
        EmbeddedResourceResource::TextResourceContents(text_res) => {
            let mime = text_res.mime_type.as_deref().unwrap_or("text/plain");
            format!(
                "[resource: {} ({}) — {} bytes]",
                text_res.uri,
                mime,
                text_res.text.len()
            )
        }
        EmbeddedResourceResource::BlobResourceContents(blob_res) => {
            let mime = blob_res
                .mime_type
                .as_deref()
                .unwrap_or("application/octet-stream");
            format!(
                "[resource: {} ({}) — {} bytes]",
                blob_res.uri,
                mime,
                blob_res.blob.len()
            )
        }
        // EmbeddedResourceResource is non_exhaustive
        _ => "[resource: unknown]".to_string(),
    }
}

/// Format a resource link content block as a descriptive placeholder.
fn format_resource_link_placeholder(
    link: &agent_client_protocol::schema::v1::ResourceLink,
) -> String {
    if link.name == link.uri {
        format!("[link: {}]", link.uri)
    } else {
        format!("[link: {} ({})]", link.name, link.uri)
    }
}

// ─── ToolCall → ContentBlock::ToolUse ───────────────────────────────────────

/// Convert an ACP `ToolCall` into a magistr `ContentBlock::ToolUse`.
///
/// Preserves the full ACP tool call structure: kind (for icon display),
/// locations (for "follow the agent"), and the standard id/name/input.
///
/// Extracts the programmatic tool name from `_meta.claudeCode.toolName`
/// when available, storing it separately from the display title so that
/// tool classification works regardless of how the ACP agent formats titles.
pub fn tool_use_from_acp(tc: &AcpToolCall) -> ContentBlock {
    ContentBlock::ToolUse {
        id: tc.tool_call_id.to_string(),
        name: if is_valid_tool_title(&tc.title) {
            tc.title.clone()
        } else {
            tool_kind_label(&tc.kind).to_string()
        },
        input: tc.raw_input.clone().unwrap_or(serde_json::Value::Null),
        kind: Some(tool_use_kind_from_acp(&tc.kind)),
        locations: locations_from_acp(&tc.locations),
        tool_name: extract_tool_name_from_meta(tc.meta.as_ref()),
    }
}

/// Extract the programmatic tool name from ACP `_meta`.
///
/// ACP agents can embed the original tool name in `_meta.claudeCode.toolName`.
/// This is agent-agnostic: any agent using this convention will be detected.
/// Falls back to `None` if the field is absent.
fn extract_tool_name_from_meta(
    meta: Option<&agent_client_protocol::schema::v1::Meta>,
) -> Option<String> {
    meta?
        .get("claudeCode")?
        .get("toolName")?
        .as_str()
        .map(str::to_string)
}

/// Check whether an ACP tool title is valid for display.
///
/// The ACP binary (Node.js) sometimes sends `"undefined"` or `"null"` as the
/// title string when the real title is missing. Treat these as invalid so we
/// fall back to a kind-based label.
fn is_valid_tool_title(title: &str) -> bool {
    !title.is_empty()
        && !title.eq_ignore_ascii_case("undefined")
        && !title.eq_ignore_ascii_case("null")
        && title.trim_matches('"') != "undefined"
        && title.trim_matches('"') != "null"
}

/// Convert ACP `ToolKind` to magistr's `ToolUseKind`.
pub fn tool_use_kind_from_acp(kind: &ToolKind) -> ToolUseKind {
    match kind {
        ToolKind::Read => ToolUseKind::Read,
        ToolKind::Edit => ToolUseKind::Edit,
        ToolKind::Delete => ToolUseKind::Delete,
        ToolKind::Move => ToolUseKind::Move,
        ToolKind::Search => ToolUseKind::Search,
        ToolKind::Execute => ToolUseKind::Execute,
        ToolKind::Think => ToolUseKind::Think,
        ToolKind::Fetch => ToolUseKind::Fetch,
        ToolKind::SwitchMode => ToolUseKind::SwitchMode,
        ToolKind::Other => ToolUseKind::Other,
        _ => ToolUseKind::Other,
    }
}

/// Convert ACP `ToolCallLocation` list to magistr's `ToolLocation` list.
fn locations_from_acp(
    locs: &[agent_client_protocol::schema::v1::ToolCallLocation],
) -> Vec<ToolLocation> {
    locs.iter()
        .map(|l| ToolLocation {
            path: l.path.display().to_string(),
            line: l.line,
        })
        .collect()
}

/// Human-readable label for an ACP `ToolKind`.
///
/// Returns a static string — no allocation needed since every variant maps
/// to a fixed label.
fn tool_kind_label(kind: &ToolKind) -> &'static str {
    match kind {
        ToolKind::Read => "Read",
        ToolKind::Edit => "Edit",
        ToolKind::Delete => "Delete",
        ToolKind::Move => "Move",
        ToolKind::Search => "Search",
        ToolKind::Execute => "Bash",
        ToolKind::Think => "Think",
        ToolKind::Fetch => "WebFetch",
        ToolKind::SwitchMode => "SwitchMode",
        ToolKind::Other => "Tool",
        // ACP is non_exhaustive — handle future variants gracefully
        _ => "Tool",
    }
}

// ─── Diff formatting ────────────────────────────────────────────────────────

/// Format an ACP `Diff` as a unified diff string.
///
/// If `old_text` is `None` (new file), all lines show as additions.
/// Otherwise produces a simple line-based unified diff with `+`/`-` prefixes.
/// Output is capped at 30 lines to avoid flooding the stream panel.
fn format_unified_diff(diff: &agent_client_protocol::schema::v1::Diff) -> String {
    let path = diff.path.display();
    let mut lines = Vec::new();
    lines.push(format!("--- {path}"));
    lines.push(format!("+++ {path}"));

    match &diff.old_text {
        None => {
            // New file — all lines are additions
            for line in diff.new_text.lines().take(28) {
                lines.push(format!("+ {line}"));
            }
            let total = diff.new_text.lines().count();
            if total > 28 {
                lines.push(format!("  ... ({} more lines)", total - 28));
            }
        }
        Some(old) => {
            // Simple line diff: show removed then added lines
            let old_lines: Vec<&str> = old.lines().collect();
            let new_lines: Vec<&str> = diff.new_text.lines().collect();
            let diff_lines = simple_line_diff(&old_lines, &new_lines);
            let capped = diff_lines.len() > 28;
            for dl in diff_lines.into_iter().take(28) {
                lines.push(dl);
            }
            if capped {
                lines.push("  ... (diff truncated)".to_string());
            }
        }
    }

    lines.join("\n")
}

/// Simple line-based diff producing `+`/`-`/` ` prefixed lines.
///
/// Uses a basic longest-common-subsequence approach for small inputs,
/// falling back to a full remove-then-add for large diffs.
fn simple_line_diff(old: &[&str], new: &[&str]) -> Vec<String> {
    // For very large diffs, just show removed + added
    if old.len() + new.len() > 200 {
        let mut result = Vec::new();
        for line in old.iter().take(14) {
            result.push(format!("- {line}"));
        }
        if old.len() > 14 {
            result.push(format!("  ... ({} lines removed)", old.len()));
        }
        for line in new.iter().take(14) {
            result.push(format!("+ {line}"));
        }
        if new.len() > 14 {
            result.push(format!("  ... ({} lines added)", new.len()));
        }
        return result;
    }

    // Simple O(n*m) LCS for reasonable-sized diffs
    let n = old.len();
    let m = new.len();
    let mut dp = vec![vec![0u32; m + 1]; n + 1];
    for i in 1..=n {
        for j in 1..=m {
            dp[i][j] = if old[i - 1] == new[j - 1] {
                dp[i - 1][j - 1] + 1
            } else {
                dp[i - 1][j].max(dp[i][j - 1])
            };
        }
    }

    // Backtrack to produce diff lines
    let mut result = Vec::new();
    let (mut i, mut j) = (n, m);
    let mut stack = Vec::new();
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && old[i - 1] == new[j - 1] {
            stack.push(format!("  {}", old[i - 1]));
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || dp[i][j - 1] >= dp[i - 1][j]) {
            stack.push(format!("+ {}", new[j - 1]));
            j -= 1;
        } else {
            stack.push(format!("- {}", old[i - 1]));
            i -= 1;
        }
    }
    stack.reverse();
    result.extend(stack);
    result
}

// ─── SessionUpdate → AgentMessage ───────────────────────────────────────────

/// Convert an ACP `SessionUpdate` to one or more `AgentMessage` values.
///
/// Most updates map 1:1, but some (like `ToolCall`) may need to be split into
/// multiple messages in future iterations. Returns a `Vec` for flexibility.
pub fn messages_from_session_update(
    update: &SessionUpdate,
    session_id: &SessionId,
) -> Vec<AgentMessage> {
    match update {
        SessionUpdate::AgentMessageChunk(chunk) => {
            let block = content_block_from_acp(&chunk.content);
            vec![AgentMessage::Assistant {
                parent_tool_use_id: None,
                message: AssistantContent {
                    model: String::new(), // ACP doesn't expose model per-chunk
                    content: vec![block],
                },
                session_id: Some(session_id.clone()),
            }]
        }

        SessionUpdate::UserMessageChunk(chunk) => {
            let block = content_block_from_acp(&chunk.content);
            let body = match &block {
                ContentBlock::Text { text } => UserBody::String(text.clone()),
                _ => UserBody::Blocks(vec![block]),
            };
            vec![AgentMessage::User {
                parent_tool_use_id: None,
                message: UserContent {
                    role: "user".to_string(),
                    content: Some(body),
                },
                session_id: Some(session_id.clone()),
            }]
        }

        SessionUpdate::AgentThoughtChunk(chunk) => {
            // Map thinking chunks to Thinking content blocks
            let text = match &chunk.content {
                AcpContentBlock::Text(tc) => tc.text.clone(),
                _ => "[thought]".to_string(),
            };
            vec![AgentMessage::Assistant {
                parent_tool_use_id: None,
                message: AssistantContent {
                    model: String::new(),
                    content: vec![ContentBlock::Thinking {
                        thinking: text,
                        signature: String::new(), // ACP thoughts have no signature
                    }],
                },
                session_id: Some(session_id.clone()),
            }]
        }

        SessionUpdate::ToolCall(tc) => {
            let block = tool_use_from_acp(tc);
            vec![AgentMessage::Assistant {
                parent_tool_use_id: None,
                message: AssistantContent {
                    model: String::new(),
                    content: vec![block],
                },
                session_id: Some(session_id.clone()),
            }]
        }

        SessionUpdate::ToolCallUpdate(tcu) => tool_call_update_to_messages(tcu, session_id),

        SessionUpdate::Plan(plan) => {
            vec![plan_to_message(plan)]
        }

        SessionUpdate::ConfigOptionUpdate(update) => {
            let options = update
                .config_options
                .iter()
                .map(config_option_from_acp)
                .collect();
            vec![AgentMessage::ConfigUpdate { options }]
        }

        // Agent slash commands — parse into structured CommandsUpdate (C7.13)
        SessionUpdate::AvailableCommandsUpdate(update) => {
            let commands = update
                .available_commands
                .iter()
                .map(|cmd| {
                    let hint = cmd.input.as_ref().and_then(|input| match input {
                        AvailableCommandInput::Unstructured(u) => Some(u.hint.clone()),
                        _ => None,
                    });
                    AgentCommand {
                        name: cmd.name.clone(),
                        description: cmd.description.clone(),
                        hint,
                    }
                })
                .collect();
            vec![AgentMessage::CommandsUpdate { commands }]
        }

        // Usage/cost update — feed into magistr's cost tracking (C7.15)
        SessionUpdate::UsageUpdate(usage) => {
            let cost_usd = usage.cost.as_ref().map(|c| c.amount);
            vec![AgentMessage::UsageUpdate {
                context_tokens_used: usage.used,
                context_window_size: usage.size,
                cost_usd,
            }]
        }

        // Mode/session-info updates are protocol-level concerns,
        // not user-visible messages. Map to System for observability.
        SessionUpdate::CurrentModeUpdate(_) | SessionUpdate::SessionInfoUpdate(_) => {
            vec![AgentMessage::System {
                subtype: "acp_protocol".to_string(),
                data: serde_json::json!({ "type": "session_update" }),
            }]
        }

        // ACP is non_exhaustive — handle future variants gracefully
        _ => {
            vec![AgentMessage::System {
                subtype: "acp_unknown".to_string(),
                data: serde_json::json!({ "type": "unknown_session_update" }),
            }]
        }
    }
}

/// Convert a `ToolCallUpdate` to one or more `AgentMessage` values.
///
/// Incremental updates (pending/in-progress) become `StreamEvent`.
/// Completed or failed updates with content also emit a `User` message
/// containing a `ToolResult` content block so the stream panel can display
/// tool output directly.
fn tool_call_update_to_messages(tcu: &ToolCallUpdate, session_id: &SessionId) -> Vec<AgentMessage> {
    let mut msgs = Vec::with_capacity(2);

    // Always emit a StreamEvent for progress tracking
    let locations_json: Vec<serde_json::Value> = tcu
        .fields
        .locations
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|l| {
            serde_json::json!({
                "path": l.path.display().to_string(),
                "line": l.line,
            })
        })
        .collect();

    // Extract structured diffs from ToolCallContent::Diff items (C7.9)
    let diffs_json: Vec<serde_json::Value> = tcu
        .fields
        .content
        .as_deref()
        .unwrap_or_default()
        .iter()
        .filter_map(|item| {
            if let ToolCallContent::Diff(diff) = item {
                Some(serde_json::json!({
                    "text": format_unified_diff(diff),
                }))
            } else {
                None
            }
        })
        .collect();

    // Extract terminal IDs from ToolCallContent::Terminal items (C7.12)
    let terminal_ids: Vec<String> = tcu
        .fields
        .content
        .as_deref()
        .unwrap_or_default()
        .iter()
        .filter_map(|item| {
            if let ToolCallContent::Terminal(term) = item {
                Some(term.terminal_id.0.to_string())
            } else {
                None
            }
        })
        .collect();

    let mut event = serde_json::json!({
        "type": "tool_call_update",
        "tool_call_id": tcu.tool_call_id.to_string(),
        "status": tcu.fields.status.as_ref().map(|s| format!("{s:?}")).unwrap_or_default(),
        "locations": locations_json,
        "diffs": diffs_json,
        "terminal_ids": terminal_ids,
    });

    // Propagate raw_input so the handler can push input lines for late-arriving inputs
    if let Some(ref input) = tcu.fields.raw_input {
        event["raw_input"] = input.clone();
    }

    // Propagate title so the handler can update the display name
    if let Some(ref title) = tcu.fields.title
        && is_valid_tool_title(title)
    {
        event["title"] = serde_json::Value::String(title.clone());
    }

    // Propagate kind so the handler can use it for input/result classification
    if let Some(ref kind) = tcu.fields.kind {
        event["kind"] = serde_json::Value::String(format!("{kind:?}"));
    }
    msgs.push(AgentMessage::StreamEvent {
        uuid: tcu.tool_call_id.to_string(),
        session_id: session_id.clone(),
        event,
        parent_tool_use_id: None,
    });

    // For completed/failed updates with content, also emit a tool_result
    let is_terminal = tcu
        .fields
        .status
        .as_ref()
        .is_some_and(|s| matches!(s, ToolCallStatus::Completed | ToolCallStatus::Failed));

    if is_terminal {
        let content_text = extract_tool_call_content(tcu);
        if let Some(text) = content_text {
            let is_error = tcu
                .fields
                .status
                .as_ref()
                .is_some_and(|s| matches!(s, ToolCallStatus::Failed));
            msgs.push(AgentMessage::User {
                parent_tool_use_id: None,
                message: UserContent {
                    role: "user".to_string(),
                    content: Some(UserBody::Blocks(vec![ContentBlock::ToolResult {
                        tool_use_id: tcu.tool_call_id.to_string(),
                        content: Some(text),
                        is_error: Some(is_error),
                    }])),
                },
                session_id: Some(session_id.clone()),
            });
        }
    }

    msgs
}

/// Extract displayable text from a `ToolCallUpdate`'s content fields.
///
/// Prefers `raw_output` (the complete tool output as JSON), falling back to
/// structured `ToolCallContent` (Content blocks, Diff text).
///
/// Filters out bogus `"undefined"` / `"null"` strings that the ACP binary
/// (Node.js) sometimes sends when the real output is absent.
fn extract_tool_call_content(tcu: &ToolCallUpdate) -> Option<String> {
    // Prefer raw_output — it's the complete unprocessed output (as JSON value)
    if let Some(ref raw) = tcu.fields.raw_output {
        let text = match raw {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Null => return None,
            other => other.to_string(),
        };
        if is_bogus_content(&text) {
            return None;
        }
        return Some(text);
    }

    // Fall back to structured content (Vec<ToolCallContent>)
    let content_items = tcu.fields.content.as_ref()?;
    let mut texts = Vec::new();

    for item in content_items {
        match item {
            ToolCallContent::Content(c) => {
                if let AcpContentBlock::Text(tc) = &c.content {
                    texts.push(tc.text.clone());
                }
            }
            ToolCallContent::Diff(diff) => {
                texts.push(format_unified_diff(diff));
            }
            ToolCallContent::Terminal(term) => {
                texts.push(format!("[terminal: {}]", term.terminal_id.0));
            }
            // ToolCallContent is non_exhaustive
            _ => {}
        }
    }

    if texts.is_empty() {
        None
    } else {
        Some(texts.join("\n"))
    }
}

/// Check whether a content string is a bogus placeholder from the ACP binary.
///
/// The Node.js ACP binary sometimes serializes JavaScript `undefined` / `null`
/// values as literal strings. These aren't real tool output.
fn is_bogus_content(text: &str) -> bool {
    let trimmed = text.trim().trim_matches('"');
    trimmed.eq_ignore_ascii_case("undefined") || trimmed == "null"
}

/// Convert an ACP `Plan` to a structured `AgentMessage::Plan`.
fn plan_to_message(plan: &Plan) -> AgentMessage {
    use agent_client_protocol::schema::v1::{
        PlanEntryPriority as AcpPri, PlanEntryStatus as AcpSt,
    };

    let entries = plan
        .entries
        .iter()
        .map(|e| {
            let priority = match e.priority {
                AcpPri::High => PlanEntryPriority::High,
                AcpPri::Medium => PlanEntryPriority::Medium,
                AcpPri::Low => PlanEntryPriority::Low,
                _ => PlanEntryPriority::Medium,
            };
            let status = match e.status {
                AcpSt::Pending => PlanEntryStatus::Pending,
                AcpSt::InProgress => PlanEntryStatus::InProgress,
                AcpSt::Completed => PlanEntryStatus::Completed,
                _ => PlanEntryStatus::Pending,
            };
            AcpPlanEntry {
                content: e.content.clone(),
                priority,
                status,
            }
        })
        .collect();

    AgentMessage::Plan { entries }
}

// ─── ConfigOption conversions ────────────────────────────────────────────────

/// Convert an ACP `SessionConfigOption` to magistr's `AcpConfigOption`.
pub fn config_option_from_acp(opt: &SessionConfigOption) -> AcpConfigOption {
    let category = opt.category.as_ref().map(|c| match c {
        SessionConfigOptionCategory::Mode => ConfigOptionCategory::Mode,
        SessionConfigOptionCategory::Model => ConfigOptionCategory::Model,
        SessionConfigOptionCategory::ThoughtLevel => ConfigOptionCategory::ThoughtLevel,
        SessionConfigOptionCategory::Other(s) => ConfigOptionCategory::Other(s.clone()),
        // Non-exhaustive
        _ => ConfigOptionCategory::Other("unknown".to_string()),
    });

    let (current_value, options) = match &opt.kind {
        SessionConfigKind::Select(select) => {
            let values = match &select.options {
                SessionConfigSelectOptions::Ungrouped(opts) => opts
                    .iter()
                    .map(|o| ConfigSelectValue {
                        value_id: o.value.0.to_string(),
                        name: o.name.clone(),
                    })
                    .collect(),
                SessionConfigSelectOptions::Grouped(groups) => groups
                    .iter()
                    .flat_map(|g| &g.options)
                    .map(|o| ConfigSelectValue {
                        value_id: o.value.0.to_string(),
                        name: o.name.clone(),
                    })
                    .collect(),
                // Non-exhaustive
                _ => Vec::new(),
            };
            (select.current_value.0.to_string(), values)
        }
        // Non-exhaustive — future option kinds (e.g., Boolean)
        _ => (String::new(), Vec::new()),
    };

    AcpConfigOption {
        id: opt.id.0.to_string(),
        name: opt.name.clone(),
        description: opt.description.clone(),
        category,
        current_value,
        options,
    }
}

/// Convert a vec of ACP `SessionConfigOption` to magistr's `AcpConfigOption` vec.
pub fn config_options_from_acp(opts: &[SessionConfigOption]) -> Vec<AcpConfigOption> {
    opts.iter().map(config_option_from_acp).collect()
}

// ─── PromptResponse → AgentMessage::Result / Cancelled ──────────────────────

/// Convert an ACP `PromptResponse` to an `AgentMessage`.
///
/// If the stop reason is `Cancelled`, produces `AgentMessage::Cancelled`.
/// Otherwise produces `AgentMessage::Result`.
pub fn message_from_prompt_response(
    response: &PromptResponse,
    session_id: &SessionId,
    duration_ms: u64,
    num_turns: u32,
    total_cost_usd: Option<f64>,
) -> AgentMessage {
    match response.stop_reason {
        StopReason::Cancelled => AgentMessage::Cancelled {
            session_id: session_id.clone(),
            reason: "session cancelled by client".to_string(),
        },
        _ => {
            let (is_error, subtype) = match response.stop_reason {
                StopReason::EndTurn => (false, "success"),
                StopReason::MaxTokens => (true, "max_tokens"),
                StopReason::MaxTurnRequests => (true, "max_turns"),
                StopReason::Refusal => (true, "refusal"),
                StopReason::Cancelled => unreachable!(),
                // ACP is non_exhaustive
                _ => (true, "unknown"),
            };

            AgentMessage::Result {
                subtype: subtype.to_string(),
                duration_ms,
                duration_api_ms: duration_ms, // ACP doesn't separate API time
                is_error,
                num_turns,
                session_id: session_id.clone(),
                total_cost_usd,
                usage: None, // Will be populated via ACP usage extension in A1.3
                result: None,
            }
        }
    }
}

// ─── ACP Error → MagistrError ──────────────────────────────────────────────────

/// Convert an ACP `Error` to a `MagistrError::Acp`.
pub fn magistr_error_from_acp(err: &AcpError) -> MagistrError {
    let code: i32 = err.code.into();
    MagistrError::Acp(format!("{} (code {})", err.message, code))
}

// ─── ToolCallStatus helpers ─────────────────────────────────────────────────

/// Check whether a `ToolCallStatus` represents a terminal state.
pub fn is_terminal_status(status: &ToolCallStatus) -> bool {
    matches!(status, ToolCallStatus::Completed)
}

/// Convert ACP `ToolCallContent` to plain text for tool result display.
pub fn tool_call_content_to_string(content: &[ToolCallContent]) -> String {
    content
        .iter()
        .map(|c| match c {
            ToolCallContent::Content(wrapper) => match &wrapper.content {
                AcpContentBlock::Text(tc) => tc.text.clone(),
                _ => "[non-text content]".to_string(),
            },
            ToolCallContent::Diff(diff) => format_unified_diff(diff),
            ToolCallContent::Terminal(term) => {
                format!("[terminal: {}]", term.terminal_id.0)
            }
            _ => "[unknown content]".to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "types.tests.rs"]
mod tests;
