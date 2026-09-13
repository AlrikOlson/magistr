//! The headless message vocabulary under the channel's classes.

use magistr_core::{AgentMessage, AssistantContent, ContentBlock};

use super::agent_event::trim_string_front;
use super::class::ObservationClass;
use super::observed::Observed;
use crate::error::{MagistrError, Result};

const TEXT: u64 = 1;
const THINKING: u64 = 2;
const USAGE: u64 = 1;
const PLAN: u64 = 2;
const CONFIG: u64 = 3;
const COMMANDS: u64 = 4;

/// The subtype of the system message that carries a lag marker.
pub(crate) const LAGGED_SUBTYPE: &str = "observation_lagged";

/// An assistant message made of one text-only block is a delta; anything
/// else — a tool use, a result, an image, several blocks — is lossless.
fn delta(message: &AssistantContent) -> Option<(u64, &str)> {
    match message.content.as_slice() {
        [ContentBlock::Text { text }] => Some((TEXT, text)),
        [ContentBlock::Thinking { thinking, .. }] => Some((THINKING, thinking)),
        _ => None,
    }
}

fn delta_mut(message: &mut AssistantContent) -> Option<&mut String> {
    match message.content.as_mut_slice() {
        [ContentBlock::Text { text }] => Some(text),
        [ContentBlock::Thinking { thinking, .. }] => Some(thinking),
        _ => None,
    }
}

impl Observed for Result<AgentMessage> {
    fn class(&self) -> ObservationClass {
        match self {
            Ok(AgentMessage::Assistant { message, .. }) => delta(message)
                .map_or(ObservationClass::Lossless, |(key, _)| {
                    ObservationClass::Text(key)
                }),
            Ok(AgentMessage::UsageUpdate { .. }) => ObservationClass::Telemetry(USAGE),
            Ok(AgentMessage::Plan { .. }) => ObservationClass::Telemetry(PLAN),
            Ok(AgentMessage::ConfigUpdate { .. }) => ObservationClass::Telemetry(CONFIG),
            Ok(AgentMessage::CommandsUpdate { .. }) => ObservationClass::Telemetry(COMMANDS),
            _ => ObservationClass::Lossless,
        }
    }

    fn text_bytes(&self) -> usize {
        match self {
            Ok(AgentMessage::Assistant { message, .. }) => {
                delta(message).map_or(0, |(_, text)| text.len())
            }
            _ => 0,
        }
    }

    fn append_text(&mut self, next: &Self) {
        if let (
            Ok(AgentMessage::Assistant { message, .. }),
            Ok(AgentMessage::Assistant { message: more, .. }),
        ) = (self, next)
            && let Some((_, more)) = delta(more)
            && let Some(text) = delta_mut(message)
        {
            text.push_str(more);
        }
    }

    fn trim_front(&mut self, bytes: usize) -> usize {
        match self {
            Ok(AgentMessage::Assistant { message, .. }) => {
                delta_mut(message).map_or(0, |text| trim_string_front(text, bytes))
            }
            _ => 0,
        }
    }

    fn lagged(bytes: usize, items: usize) -> Self {
        Ok(AgentMessage::System {
            subtype: LAGGED_SUBTYPE.to_owned(),
            data: serde_json::json!({ "bytes": bytes, "items": items }),
        })
    }

    fn overflowed(pending: usize) -> Self {
        Err(MagistrError::Acp(format!(
            "observation overflowed: {pending} messages were waiting for the client; observation ended, the session stays resident"
        )))
    }
}
