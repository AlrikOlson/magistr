//! Output renderer traits.
//!
//! Three traits form the renderer hierarchy:
//!
//! - [`TextRenderer`] — message-level output (success, error, warning, info).
//! - [`DataRenderer`] — structured data output (table, json, key-value, list).
//! - [`OutputRenderer`] — combines both with dispatch and format metadata.
//!
//! Command handlers return [`CommandResult`] instead of performing I/O directly.
//! The caller picks a renderer (`Plain`, `Json`, `Silent`) and renders the result.

use std::io::Write;

use super::command_result::CommandResult;

// ── TextRenderer ─────────────────────────────────────────────────────

/// Message-level output: success, error, warning, and info messages.
pub trait TextRenderer {
    /// Render a success message.
    fn success(&self, w: &mut dyn Write, msg: &str) -> std::io::Result<()>;

    /// Render an error message.
    fn error(&self, w: &mut dyn Write, msg: &str) -> std::io::Result<()>;

    /// Render a warning message.
    fn warning(&self, w: &mut dyn Write, msg: &str) -> std::io::Result<()>;

    /// Render an informational message.
    fn info(&self, w: &mut dyn Write, msg: &str) -> std::io::Result<()>;
}

// ── DataRenderer ─────────────────────────────────────────────────────

/// Structured data output: tables, JSON, key-value pairs, and lists.
pub trait DataRenderer {
    /// Render a table with headers and rows.
    fn table(
        &self,
        w: &mut dyn Write,
        headers: &[String],
        rows: &[Vec<String>],
    ) -> std::io::Result<()>;

    /// Render raw JSON data.
    fn json(&self, w: &mut dyn Write, data: &serde_json::Value) -> std::io::Result<()>;

    /// Render key-value pairs.
    fn kv(&self, w: &mut dyn Write, pairs: &[(String, String)]) -> std::io::Result<()>;

    /// Render a list of items.
    fn list(&self, w: &mut dyn Write, items: &[String]) -> std::io::Result<()>;
}

// ── OutputRenderer ───────────────────────────────────────────────────

/// Full output renderer — combines [`TextRenderer`] + [`DataRenderer`] with
/// format metadata and `CommandResult` dispatch.
pub trait OutputRenderer: TextRenderer + DataRenderer {
    /// Whether this renderer produces machine-readable output (JSON).
    ///
    /// Command handlers with complex plain-text formatting can use this
    /// to choose between rich terminal output and structured data.
    fn is_machine_readable(&self) -> bool {
        false
    }

    /// Render a `CommandResult` through the appropriate method.
    fn render(&self, w: &mut dyn Write, result: &CommandResult) -> std::io::Result<()> {
        match result {
            CommandResult::Success(msg) => self.success(w, msg),
            CommandResult::Error(msg) => self.error(w, msg),
            CommandResult::Warning(msg) => self.warning(w, msg),
            CommandResult::Info(msg) => self.info(w, msg),
            CommandResult::KeyValues(pairs) => self.kv(w, pairs),
            CommandResult::Table { headers, rows } => self.table(w, headers, rows),
            CommandResult::List(items) => self.list(w, items),
            CommandResult::Json(data) => self.json(w, data),
            CommandResult::Empty => Ok(()),
        }
    }
}

#[cfg(test)]
#[path = "renderer.tests.rs"]
mod tests;
