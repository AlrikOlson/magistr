//! The request verbs an attached client can ask its owner for.

use magistr_workflow::request::domain::RequestKind;
use serde::{Deserialize, Serialize};

/// One request-inbox mutation, argument for argument the
/// [`NativeEngines`](super::NativeEngines) verb of the same name.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "verb", rename_all = "snake_case")]
pub enum RequestCommand {
    /// `request_capture`.
    Capture {
        kind: RequestKind,
        from: String,
        body: String,
    },
    /// `request_research`.
    Research {
        id: String,
        summary: String,
        confidence: f64,
        sources: Vec<String>,
        note: Option<u32>,
    },
    /// `request_promote`.
    Promote { id: String },
    /// `request_dismiss`.
    Dismiss { id: String },
    /// `request_snooze`.
    Snooze { id: String, minutes: i64 },
}
