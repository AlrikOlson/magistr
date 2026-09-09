use serde::{Deserialize, Serialize};

/// One frame off the think-and-ship broadcast socket.
///
/// The wire is NDJSON: every line is a JSON object with a `family` tag
/// (`think` | `ship` | `roadmap` | `signal`) flattened onto a typed payload
/// whose discriminator is `type` (`chunk_changed`, `step_appended`,
/// `action_recorded`, `signal_captured`, …). The payload is kept whole so a
/// consumer can deserialize it into the family's own frame type when it
/// wants more than the two tags.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeFrame {
    /// Which engine emitted it.
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

    /// Whether this frame came from the roadmap engine.
    #[must_use]
    pub fn is_roadmap(&self) -> bool {
        self.family == "roadmap"
    }
}

#[cfg(test)]
#[path = "broadcast_frame.tests.rs"]
mod tests;
