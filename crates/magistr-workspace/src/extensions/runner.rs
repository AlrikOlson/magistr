use super::{ExtensionLane, PluginGuestPolicy, RunnerInput};
use crate::{
    contracts::{ErrorKind, WorkspaceError},
    worker::{WorkerGuestEnd, WorkerRunReport},
};

/// A bundled runner adapter served by a WASM component on the public
/// `magistr:runner@0.1.0` interface: `can-parse` then `parse-output`, both
/// inside the worker. `Ok(None)` is the guest's honest "no structure"; an
/// `Err` is a lane or guest problem the run tool degrades from by falling
/// back to its bundled floor (owned by the execution port).
pub struct RunnerAdapter<'a> {
    name: String,
    guest: Vec<u8>,
    lane: &'a dyn ExtensionLane,
    policy: PluginGuestPolicy,
}

impl<'a> RunnerAdapter<'a> {
    /// Defaults mirror the bundled runner manifest's limits.
    pub fn new(name: impl Into<String>, guest: Vec<u8>, lane: &'a dyn ExtensionLane) -> Self {
        Self {
            name: name.into(),
            guest,
            lane,
            policy: PluginGuestPolicy::clamped(Some(200_000_000), Some(32 * 1024 * 1024)),
        }
    }

    #[must_use]
    pub fn with_policy(mut self, policy: PluginGuestPolicy) -> Self {
        self.policy = policy;
        self
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    pub async fn parse(
        &self,
        input: RunnerInput,
    ) -> Result<Option<WorkerRunReport>, WorkspaceError> {
        let command = input.command.clone();
        let outcome = self
            .lane
            .parse_run_output(self.guest.clone(), input, Vec::new(), self.policy)
            .await?;
        if outcome.end != WorkerGuestEnd::Ok {
            return Err(WorkspaceError::new(
                ErrorKind::PluginCrashed {
                    plugin: self.name.clone(),
                },
                format!(
                    "runner {} ended {}",
                    self.name,
                    super::PluginInvokeOutcome::ended(outcome.end, outcome.fuel_used).reason()
                ),
            ));
        }
        if !outcome.claimed {
            return Err(WorkspaceError::new(
                ErrorKind::CommandNotDeclared,
                format!("runner {} did not claim {command:?}", self.name),
            ));
        }
        Ok(outcome.report)
    }
}

#[cfg(test)]
#[path = "runner.tests.rs"]
mod tests;
