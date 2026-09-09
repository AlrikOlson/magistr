use crate::{ApplicationError, ProjectScope};

use super::{CheckExecution, CheckPort, CheckRecording, RunCheck};

/// Project-bound access to configured quality gate execution.
pub struct Checks<'a> {
    scope: &'a ProjectScope,
    port: &'a mut dyn CheckPort,
}

impl<'a> Checks<'a> {
    pub fn new(scope: &'a ProjectScope, port: &'a mut dyn CheckPort) -> Self {
        Self { scope, port }
    }

    pub async fn run(&mut self, request: RunCheck) -> Result<CheckExecution, ApplicationError> {
        self.scope.ensure_command(&request.command)?;
        request.command.ensure_run(request.task.run())?;
        let context = request.command.clone();
        let gate = request.gate.clone();
        let task = request.task.clone();
        let execution = self.port.run_check(request).await?;
        context.validate_receipt(&execution.receipt)?;
        if execution.gate != gate
            || execution.outcome.kind != gate.kind()
            || !execution.outcome.status.is_terminal()
        {
            return Err(ApplicationError::InvalidResponse {
                reason: "check execution must return the requested gate's terminal outcome".into(),
            });
        }
        if matches!(&execution.recording, CheckRecording::Recorded { task: recorded_task, check_key } if recorded_task != &task || check_key.trim().is_empty())
        {
            return Err(ApplicationError::InvalidResponse {
                reason: "recorded check must reference the requested task and a nonempty native check key".into(),
            });
        }
        Ok(execution)
    }
}

#[cfg(test)]
#[path = "service.tests.rs"]
mod tests;
