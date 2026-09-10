//! The reasoning verbs an attached client can ask its owner for.

use serde::{Deserialize, Serialize};
use think_and_ship::think::domain::ThinkStep;

/// One reasoning-trace mutation, argument for argument the
/// [`NativeEngines`](super::NativeEngines) verb of the same name.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "verb", rename_all = "snake_case")]
pub enum ThinkCommand {
    /// `think_record_step`. Boxed: a step is by far the largest payload
    /// on the wire.
    RecordStep { step: Box<ThinkStep> },
    /// `think_pin_step`.
    PinStep { step_number: u32, pinned: bool },
    /// `think_checkpoint`.
    Checkpoint,
}
