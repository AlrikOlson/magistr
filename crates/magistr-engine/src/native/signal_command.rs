//! The signal verbs an attached client can ask its owner for.

use magistr_workflow::signal::domain::SignalKind;
use serde::{Deserialize, Serialize};

/// One signal-inbox mutation, argument for argument the
/// [`NativeEngines`](super::NativeEngines) verb of the same name.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "verb", rename_all = "snake_case")]
pub enum SignalCommand {
    /// `signal_capture`.
    Capture {
        kind: SignalKind,
        from: String,
        body: String,
    },
    /// `signal_research`.
    Research {
        id: String,
        summary: String,
        confidence: f64,
        sources: Vec<String>,
        think_step: Option<u32>,
    },
    /// `signal_promote`.
    Promote { id: String },
    /// `signal_ignore`.
    Ignore { id: String },
    /// `signal_snooze`.
    Snooze { id: String, minutes: i64 },
}
