//! The socket line, and the one place it is read as strings.

use serde::{Deserialize, Serialize};

use crate::{EPOCH_KEY, Family, SEQ_KEY};

use super::event_cursor::EventCursor;
use super::native_event::NativeEvent;
use super::owner_generation::OwnerGeneration;

/// One line off the native store's broadcast socket, as the wire carries
/// it: a JSON object with a `family` tag flattened onto a typed payload
/// whose discriminator is `type`, plus the publisher's `seq` and `epoch`
/// stamps. This is the current serialized shape at the transport edge, and
/// [`NativeFrame::decode`] is the only reader of its strings: everything
/// past it holds a [`NativeEvent`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeFrame {
    /// Which family emitted it, as the wire spells it.
    pub family: String,
    /// The payload's `type` discriminator.
    pub kind: String,
    /// The whole frame object, tags included.
    pub payload: serde_json::Value,
}

impl NativeFrame {
    /// Parse one socket line. Lines that are not a JSON object carrying a
    /// string `family` are dropped (`None`); a missing `type` is allowed and
    /// reads as an empty kind.
    #[must_use]
    pub fn parse_line(line: &str) -> Option<Self> {
        let payload: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
        let obj = payload.as_object()?;
        let family = obj.get("family")?.as_str()?.to_owned();
        let kind = obj
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or_default()
            .to_owned();
        Some(Self {
            family,
            kind,
            payload,
        })
    }

    /// Shared planning events reach every view; task and command events reach
    /// only the execution that produced them. Unbound task events are refused.
    #[must_use]
    pub fn is_for_execution(&self, key: &str) -> bool {
        !matches!(self.family.as_str(), "ship" | "magistr")
            || self
                .payload
                .get("execution_key")
                .and_then(serde_json::Value::as_str)
                == Some(key)
    }

    /// The typed event this line carries, and where it stands in its
    /// publisher's sequence. `None` for a family or a kind this build does
    /// not know, or a payload that is not the shape its kind promises — a
    /// newer publisher's frame is dropped here rather than half-read. The
    /// cursor is `None` when the publisher stamped no epoch or no
    /// sequence, or an epoch no owner could have minted.
    #[must_use]
    pub fn decode(self) -> Option<(NativeEvent, Option<EventCursor>)> {
        let cursor = cursor_of(&self.payload);
        let event = match Family::parse(&self.family)? {
            Family::Think => NativeEvent::Think(serde_json::from_value(self.payload).ok()?),
            Family::Ship => NativeEvent::Ship(serde_json::from_value(self.payload).ok()?),
            Family::Roadmap => NativeEvent::Roadmap(serde_json::from_value(self.payload).ok()?),
            Family::Signal => NativeEvent::Signal(serde_json::from_value(self.payload).ok()?),
            Family::Magistr => NativeEvent::Magistr(serde_json::from_value(self.payload).ok()?),
        };
        Some((event, cursor))
    }
}

fn cursor_of(payload: &serde_json::Value) -> Option<EventCursor> {
    let epoch = OwnerGeneration::admitted(payload.get(EPOCH_KEY)?.as_u64()?).ok()?;
    let sequence = payload.get(SEQ_KEY)?.as_u64()?;
    Some(EventCursor { epoch, sequence })
}
