//! The cockpit's event vocabulary under the channel's classes.

use magistr_cockpit::agent::AgentEvent;

use super::class::ObservationClass;
use super::observed::Observed;

const TEXT: u64 = 1;
const THOUGHT: u64 = 2;
const USAGE: u64 = 1;
const PLAN: u64 = 2;

/// One terminal's bytes coalesce only with the same terminal's.
fn terminal_key(terminal_id: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    terminal_id.hash(&mut hasher);
    // Above the fixed keys, so no terminal collides with text or thought.
    hasher.finish() | (1 << 63)
}

/// Drop up to `bytes` from the front of `text` on a char boundary.
pub(super) fn trim_string_front(text: &mut String, bytes: usize) -> usize {
    let at = text
        .char_indices()
        .map(|(index, _)| index)
        .take_while(|index| *index <= bytes)
        .last()
        .unwrap_or(0)
        .min(text.len());
    let at = if at < bytes && at < text.len() {
        // The boundary just past `bytes`, so at least `bytes` go.
        text[at..]
            .char_indices()
            .nth(1)
            .map_or(text.len(), |(next, _)| at + next)
    } else {
        at
    };
    text.drain(..at);
    at
}

impl Observed for AgentEvent {
    fn class(&self) -> ObservationClass {
        match self {
            Self::Text(_) => ObservationClass::Text(TEXT),
            Self::Thought(_) => ObservationClass::Text(THOUGHT),
            Self::Terminal { terminal_id, .. } => ObservationClass::Text(terminal_key(terminal_id)),
            Self::Usage { .. } => ObservationClass::Telemetry(USAGE),
            Self::Plan { .. } => ObservationClass::Telemetry(PLAN),
            _ => ObservationClass::Lossless,
        }
    }

    fn text_bytes(&self) -> usize {
        match self {
            Self::Text(text) | Self::Thought(text) => text.len(),
            Self::Terminal { bytes, .. } => bytes.len(),
            _ => 0,
        }
    }

    fn append_text(&mut self, next: &Self) {
        match (self, next) {
            (Self::Text(text), Self::Text(more)) | (Self::Thought(text), Self::Thought(more)) => {
                text.push_str(more);
            }
            (Self::Terminal { bytes, .. }, Self::Terminal { bytes: more, .. }) => {
                bytes.extend_from_slice(more);
            }
            _ => {}
        }
    }

    fn trim_front(&mut self, bytes: usize) -> usize {
        match self {
            Self::Text(text) | Self::Thought(text) => trim_string_front(text, bytes),
            Self::Terminal { bytes: raw, .. } => {
                let at = bytes.min(raw.len());
                raw.drain(..at);
                at
            }
            _ => 0,
        }
    }

    fn lagged(bytes: usize, items: usize) -> Self {
        Self::Lagged { bytes, items }
    }

    fn overflowed(pending: usize) -> Self {
        Self::Error(format!(
            "observation overflowed: {pending} events were waiting for the cockpit; observation ended, the session stays resident"
        ))
    }
}
