//! From the protocol's events to the cockpit's.

use agent_client_protocol::schema::v1::{
    ContentBlock, Cost, PlanEntryStatus, SessionConfigKind, SessionConfigOption,
    SessionConfigOptionCategory, SessionUpdate, StopReason, ToolCallContent, ToolCallLocation,
    ToolCallStatus, ToolKind,
};
use magistr_acp::AcpEvent;

use super::edit_diff::EditDiff;
use super::event::AgentEvent;
use super::image_block::ImageBlock;
use super::tool_record::ToolRecord;
use super::tool_status::ToolStatus;
use super::tool_verb::ToolVerb;
use super::turn_end::TurnEnd;

/// The cockpit's reading of one transport event; nothing is dropped —
/// an update this cockpit has no words for becomes [`AgentEvent::System`]
/// with the update's name, a transport error [`AgentEvent::Error`].
#[must_use]
pub fn translate(event: &magistr_acp::Result<AcpEvent>) -> AgentEvent {
    let event = match event {
        Ok(event) => event,
        Err(e) => return AgentEvent::Error(e.to_string()),
    };
    match event {
        AcpEvent::Initialized {
            session_id,
            config_options,
            ..
        } => AgentEvent::Initialized {
            session_id: session_id.clone(),
            model: current_model(config_options.as_deref()),
        },
        AcpEvent::SessionUpdate { update, .. } => session_update(update),
        AcpEvent::PromptResponse { response, .. } => {
            AgentEvent::TurnEnded(turn_end(&response.stop_reason))
        }
        // Extension outcomes belong to the admitting session owner. A raw
        // response alone cannot establish a cockpit turn outcome.
        AcpEvent::ExtensionResponse { .. } => AgentEvent::System {
            subtype: "extension_response".to_owned(),
        },
        AcpEvent::Terminal { terminal_id, bytes } => AgentEvent::Terminal {
            terminal_id: terminal_id.clone(),
            bytes: bytes.clone(),
        },
        AcpEvent::System { subtype, .. } => AgentEvent::System {
            subtype: subtype.clone(),
        },
    }
}

fn session_update(update: &SessionUpdate) -> AgentEvent {
    match update {
        SessionUpdate::AgentMessageChunk(chunk) => {
            block(&chunk.content).unwrap_or_else(|| AgentEvent::Text(text(&chunk.content)))
        }
        SessionUpdate::AgentThoughtChunk(chunk) => {
            block(&chunk.content).unwrap_or_else(|| AgentEvent::Thought(text(&chunk.content)))
        }
        SessionUpdate::UserMessageChunk(chunk) => {
            block(&chunk.content).unwrap_or_else(|| AgentEvent::UserText(text(&chunk.content)))
        }
        SessionUpdate::ToolCall(call) => AgentEvent::ToolCall {
            id: call.tool_call_id.to_string(),
            title: call.title.clone(),
            verb: tool_verb(call.kind),
            status: tool_status(&call.status),
            diff: edit_diff(&call.content),
            images: tool_images(&call.content),
            record: tool_record(
                Some(call.kind),
                &call.content,
                &call.locations,
                call.raw_input.as_ref(),
                call.raw_output.as_ref(),
            ),
        },
        SessionUpdate::ToolCallUpdate(update) => AgentEvent::ToolCallUpdate {
            id: update.tool_call_id.to_string(),
            title: update.fields.title.clone(),
            status: update.fields.status.as_ref().map(tool_status),
            diff: update.fields.content.as_deref().and_then(edit_diff),
            duration_ms: None,
            images: update
                .fields
                .content
                .as_deref()
                .map(tool_images)
                .unwrap_or_default(),
            record: tool_record(
                update.fields.kind,
                update.fields.content.as_deref().unwrap_or_default(),
                update.fields.locations.as_deref().unwrap_or_default(),
                update.fields.raw_input.as_ref(),
                update.fields.raw_output.as_ref(),
            ),
        },
        SessionUpdate::Plan(plan) => AgentEvent::Plan {
            entries: plan.entries.len(),
            completed: plan
                .entries
                .iter()
                .filter(|e| matches!(e.status, PlanEntryStatus::Completed))
                .count(),
        },
        SessionUpdate::UsageUpdate(usage) => AgentEvent::Usage {
            used: usage.used,
            size: usage.size,
            cost_cents: usage.cost.as_ref().and_then(cost_cents),
            // The wire carries no clock; the session stamps its own.
            elapsed_secs: None,
        },
        SessionUpdate::AvailableCommandsUpdate(_) => system("available_commands"),
        SessionUpdate::CurrentModeUpdate(_) => system("current_mode"),
        SessionUpdate::ConfigOptionUpdate(_) => system("config_options"),
        SessionUpdate::SessionInfoUpdate(_) => system("session_info"),
        _ => system("session_update"),
    }
}

/// The model the session opened on, as the ACP model state names it:
/// the current value of the `Model`-category select among the
/// session's config options. `None` when the agent advertises no
/// model option, or one that is not a select.
fn current_model(options: Option<&[SessionConfigOption]>) -> Option<String> {
    options?
        .iter()
        .find(|option| option.category == Some(SessionConfigOptionCategory::Model))
        .and_then(|option| match &option.kind {
            SessionConfigKind::Select(select) => Some(select.current_value.to_string()),
            _ => None,
        })
}

/// The first diff among a tool call's content blocks, as the edit
/// card's input; `None` when the call carried no diff.
fn edit_diff(content: &[ToolCallContent]) -> Option<EditDiff> {
    content.iter().find_map(|item| match item {
        ToolCallContent::Diff(diff) => Some(EditDiff {
            path: diff.path.display().to_string(),
            old_text: diff.old_text.clone().unwrap_or_default(),
            new_text: diff.new_text.clone(),
        }),
        _ => None,
    })
}

/// Everything a tool call (or an update to one) said past its row, as
/// the [`ToolRecord`] its line keeps: the kind when the wire named one,
/// the text content blocks in order, the one terminal it embeds, the
/// locations it touched and the raw input and output — bounded at the
/// record's cap before it leaves the boundary.
fn tool_record(
    kind: Option<ToolKind>,
    content: &[ToolCallContent],
    locations: &[ToolCallLocation],
    input: Option<&serde_json::Value>,
    output: Option<&serde_json::Value>,
) -> Box<ToolRecord> {
    let record = ToolRecord {
        kind: kind.map(tool_verb),
        input: input.cloned(),
        output: output.cloned(),
        text: content
            .iter()
            .filter_map(|item| match item {
                ToolCallContent::Content(item) => match &item.content {
                    ContentBlock::Text(text) => Some(text.text.clone()),
                    _ => None,
                },
                _ => None,
            })
            .collect(),
        locations: locations
            .iter()
            .map(|location| (location.path.clone(), location.line))
            .collect(),
        terminal: content.iter().find_map(|item| match item {
            ToolCallContent::Terminal(terminal) => Some(terminal.terminal_id.to_string()),
            _ => None,
        }),
        truncated_from: None,
    };
    Box::new(record.bounded())
}

/// The image blocks among a tool call's content, in the order the call
/// listed them — a screenshot a browser or vision tool returned rides
/// here rather than on a message chunk. Empty when the call carried
/// none, which is every call that is not a picture.
fn tool_images(content: &[ToolCallContent]) -> Vec<ImageBlock> {
    content
        .iter()
        .filter_map(|item| match item {
            ToolCallContent::Content(item) => image_block(&item.content),
            _ => None,
        })
        .collect()
}

/// The event a non-text content block becomes, when it is one the
/// stream shows as more than words. Only images qualify today; every
/// other block falls through to [`text`]'s bracketed kind.
fn block(block: &ContentBlock) -> Option<AgentEvent> {
    image_block(block).map(AgentEvent::Image)
}

/// One content block as an [`ImageBlock`], when it is an image — the
/// one reading of the protocol's image block, whichever list it came
/// from.
fn image_block(block: &ContentBlock) -> Option<ImageBlock> {
    match block {
        ContentBlock::Image(image) => Some(ImageBlock {
            mime_type: image.mime_type.clone(),
            data: image.data.clone(),
            uri: image.uri.clone(),
        }),
        _ => None,
    }
}

/// A priced usage update's cost in cents — the unit the rail's budget
/// keeps, so two frames of one spend can never differ by a float's
/// residue. Only dollars count: the cap is in USD, and a figure in any
/// other currency would fill the meter in the wrong unit.
fn cost_cents(cost: &Cost) -> Option<u64> {
    (cost.currency == "USD" && cost.amount.is_finite() && cost.amount >= 0.0)
        .then(|| (cost.amount * 100.0).round() as u64)
}

fn system(subtype: &str) -> AgentEvent {
    AgentEvent::System {
        subtype: subtype.to_owned(),
    }
}

/// The text of a content block; non-text blocks read as a bracketed kind.
fn text(block: &ContentBlock) -> String {
    match block {
        ContentBlock::Text(text) => text.text.clone(),
        ContentBlock::Image(_) => "[image]".to_owned(),
        ContentBlock::Audio(_) => "[audio]".to_owned(),
        ContentBlock::Resource(_) | ContentBlock::ResourceLink(_) => "[resource]".to_owned(),
        _ => "[content]".to_owned(),
    }
}

fn tool_verb(kind: ToolKind) -> ToolVerb {
    match kind {
        ToolKind::Read => ToolVerb::Read,
        ToolKind::Edit => ToolVerb::Edit,
        ToolKind::Delete => ToolVerb::Delete,
        ToolKind::Move => ToolVerb::Move,
        ToolKind::Search => ToolVerb::Search,
        ToolKind::Execute => ToolVerb::Run,
        ToolKind::Think => ToolVerb::Think,
        ToolKind::Fetch => ToolVerb::Fetch,
        ToolKind::SwitchMode => ToolVerb::Mode,
        _ => ToolVerb::Tool,
    }
}

fn tool_status(status: &ToolCallStatus) -> ToolStatus {
    match status {
        ToolCallStatus::InProgress => ToolStatus::InProgress,
        ToolCallStatus::Completed => ToolStatus::Completed,
        ToolCallStatus::Failed => ToolStatus::Failed,
        _ => ToolStatus::Pending,
    }
}

fn turn_end(reason: &StopReason) -> TurnEnd {
    match reason {
        StopReason::EndTurn => TurnEnd::EndTurn,
        StopReason::MaxTokens => TurnEnd::MaxTokens,
        StopReason::MaxTurnRequests => TurnEnd::MaxTurnRequests,
        StopReason::Refusal => TurnEnd::Refusal,
        StopReason::Cancelled => TurnEnd::Cancelled,
        _ => TurnEnd::Other,
    }
}

#[cfg(test)]
#[path = "translate.tests.rs"]
mod tests;
