//! JSON renderer for machine-readable output.
//!
//! Wraps all output in the standard envelope format: `{ "status": "ok", "data": {...} }`.
//! Messages are serialized as JSON objects with a `message` field.

use std::io::Write;

use super::envelope;
use super::renderer::{DataRenderer, OutputRenderer, TextRenderer};

/// Machine-readable JSON output renderer.
///
/// All output goes to the provided writer as pretty-printed JSON.
/// Messages are wrapped in the standard envelope format.
#[derive(Debug, Default)]
pub struct JsonRenderer;

impl JsonRenderer {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Write a JSON value to the writer with a trailing newline.
    fn write_json(&self, w: &mut dyn Write, value: &serde_json::Value) -> std::io::Result<()> {
        let formatted = serde_json::to_string_pretty(value).map_err(std::io::Error::other)?;
        writeln!(w, "{formatted}")
    }

    /// Wrap data in the standard success envelope and write it.
    fn write_envelope(&self, w: &mut dyn Write, data: serde_json::Value) -> std::io::Result<()> {
        let envelope = serde_json::json!({
            "status": "ok",
            "data": data,
        });
        self.write_json(w, &envelope)
    }
}

impl TextRenderer for JsonRenderer {
    fn success(&self, w: &mut dyn Write, msg: &str) -> std::io::Result<()> {
        self.write_envelope(w, serde_json::json!({ "message": msg, "level": "success" }))
    }

    fn error(&self, w: &mut dyn Write, msg: &str) -> std::io::Result<()> {
        let env = envelope::error(msg);
        let json = serde_json::to_string_pretty(&env).map_err(std::io::Error::other)?;
        writeln!(w, "{json}")
    }

    fn warning(&self, w: &mut dyn Write, msg: &str) -> std::io::Result<()> {
        self.write_envelope(w, serde_json::json!({ "message": msg, "level": "warning" }))
    }

    fn info(&self, w: &mut dyn Write, msg: &str) -> std::io::Result<()> {
        self.write_envelope(w, serde_json::json!({ "message": msg, "level": "info" }))
    }
}

impl DataRenderer for JsonRenderer {
    fn table(
        &self,
        w: &mut dyn Write,
        headers: &[String],
        rows: &[Vec<String>],
    ) -> std::io::Result<()> {
        let json_rows: Vec<serde_json::Value> = rows
            .iter()
            .map(|row| {
                let obj: serde_json::Map<String, serde_json::Value> = headers
                    .iter()
                    .zip(row.iter())
                    .map(|(h, v)| (h.clone(), serde_json::Value::String(v.clone())))
                    .collect();
                serde_json::Value::Object(obj)
            })
            .collect();
        self.write_envelope(w, serde_json::json!(json_rows))
    }

    fn json(&self, w: &mut dyn Write, data: &serde_json::Value) -> std::io::Result<()> {
        self.write_envelope(w, data.clone())
    }

    fn kv(&self, w: &mut dyn Write, pairs: &[(String, String)]) -> std::io::Result<()> {
        let obj: serde_json::Map<String, serde_json::Value> = pairs
            .iter()
            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
            .collect();
        self.write_envelope(w, serde_json::Value::Object(obj))
    }

    fn list(&self, w: &mut dyn Write, items: &[String]) -> std::io::Result<()> {
        let json_items: Vec<serde_json::Value> = items
            .iter()
            .map(|s| serde_json::Value::String(s.clone()))
            .collect();
        self.write_envelope(w, serde_json::json!(json_items))
    }
}

impl OutputRenderer for JsonRenderer {
    fn is_machine_readable(&self) -> bool {
        true
    }
}

#[cfg(test)]
#[path = "json.tests.rs"]
mod tests;
