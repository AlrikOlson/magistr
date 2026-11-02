//! Typed mutation records; emission belongs to the workflow host.

use super::Request;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RequestFrame {
    /// A new request was captured.
    RequestCaptured { request: Request },
    /// A non-terminal change (research/link/raise/snooze/promote).
    RequestChanged { request: Request },
    /// Terminal: the request was dismissed.
    RequestDismissed { request: Request },
}
