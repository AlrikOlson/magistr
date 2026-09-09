//! Structured result type for command handlers.

/// Structured result from a command handler.
///
/// Handlers produce data; renderers decide how to display it.
#[derive(Debug)]
pub enum CommandResult {
    /// A success message (e.g., "Scaffold created").
    Success(String),

    /// An error message.
    Error(String),

    /// A warning message.
    Warning(String),

    /// An informational message.
    Info(String),

    /// Structured key-value pairs (e.g., detection results).
    KeyValues(Vec<(String, String)>),

    /// Tabular data with headers and rows.
    Table {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
    },

    /// A simple list of items (e.g., available plugins, detected languages).
    List(Vec<String>),

    /// Raw JSON-serializable data (for commands that already produce structured output).
    Json(serde_json::Value),

    /// No output needed (side-effect-only commands like file writes).
    Empty,
}
