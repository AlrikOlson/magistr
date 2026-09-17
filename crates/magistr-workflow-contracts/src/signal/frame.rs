//! Typed mutation records; emission belongs to the workflow host.

use super::Signal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SignalFrame {
    /// A new signal was captured.
    SignalCaptured { signal: Signal },
    /// A non-terminal change (research/link/surface/snooze/promote).
    SignalChanged { signal: Signal },
    /// Terminal: the signal was dismissed (ignored).
    SignalDismissed { signal: Signal },
}
