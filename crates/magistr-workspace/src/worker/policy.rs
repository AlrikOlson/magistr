use serde::{Deserialize, Serialize};

use super::WorkerError;

/// A requested sandbox budget, clamped to the existing script host ceilings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerPolicy {
    pub fuel: u64,
    pub memory_cap_bytes: usize,
}

impl WorkerPolicy {
    /// Workers never increase the request's limits. These ceilings match the
    /// adopted script host's 1 billion fuel / 256 MiB policy.
    pub(crate) fn admitted(self) -> Result<crate::sandbox::SandboxPolicy, WorkerError> {
        if self.fuel == 0 || self.memory_cap_bytes == 0 {
            return Err(WorkerError::Remote {
                code: "MALFORMED_FRAME".to_owned(),
                message: "sandbox fuel and memory limits must be positive".to_owned(),
            });
        }
        Ok(crate::sandbox::SandboxPolicy {
            fuel: self.fuel.min(1_000_000_000),
            memory_cap_bytes: self.memory_cap_bytes.min(256 * 1024 * 1024),
        })
    }
}
