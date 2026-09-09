//! NDJSON event stream renderer for headless mode.
//!
//! Produces one JSON object per line to stdout. Each line contains:
//! `{"timestamp": "...", "event": "...", "data": {...}}`

use std::io::Write;

use chrono::Utc;
use serde_json::json;

use super::event::HeadlessEvent;
use super::renderer::HeadlessRenderer;

/// NDJSON (newline-delimited JSON) event stream renderer.
///
/// Each event is serialized as a single JSON object on one line,
/// prefixed with an ISO 8601 timestamp. This format is easy to parse
/// with `jq`, Python, or any JSON library.
#[derive(Debug, Default)]
pub struct JsonStreamRenderer;

impl JsonStreamRenderer {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl HeadlessRenderer for JsonStreamRenderer {
    fn emit(&self, w: &mut dyn Write, event: HeadlessEvent) -> std::io::Result<()> {
        let event_json = serde_json::to_value(&event)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let envelope = json!({
            "timestamp": Utc::now().to_rfc3339(),
            "event": event_json["event"],
            "data": event_json["data"],
        });

        let line = serde_json::to_string(&envelope)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        writeln!(w, "{line}")
    }
}

#[cfg(test)]
#[path = "json_stream.tests.rs"]
mod tests;
