use core::num::NonZeroU32;
use serde::{Deserialize, Serialize};

use super::CommandId;

/// A one-based execution attempt scoped to the command it retries.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AttemptId {
    command: CommandId,
    number: NonZeroU32,
}

impl AttemptId {
    pub fn new(command: CommandId, number: NonZeroU32) -> Self {
        Self { command, number }
    }

    pub fn command(&self) -> &CommandId {
        &self.command
    }

    pub fn number(&self) -> NonZeroU32 {
        self.number
    }
}
