use super::{RunFuture, RunOutput, RunReport};
use crate::contracts::WorkspaceError;

/// A runner parser served by a plugin guest, consulted before the bundled
/// parsers in trust order. `Ok(None)` is the guest's honest "claimed, no
/// structure"; an `Err` with `COMMAND_NOT_DECLARED` means the guest did not
/// claim the line; any other `Err` is a lane or guest failure the run tool
/// degrades from with a note.
pub trait PluginRunnerLane: Send + Sync {
    fn name(&self) -> &str;
    fn parse(&self, output: RunOutput) -> RunFuture<'_, Result<Option<RunReport>, WorkspaceError>>;
}

#[cfg(feature = "sandbox")]
impl PluginRunnerLane for crate::extensions::RunnerAdapter<'_> {
    fn name(&self) -> &str {
        crate::extensions::RunnerAdapter::name(self)
    }

    fn parse(&self, output: RunOutput) -> RunFuture<'_, Result<Option<RunReport>, WorkspaceError>> {
        Box::pin(async move {
            let input = crate::extensions::RunnerInput {
                command: output.command,
                exit_code: output.exit_code,
                stdout: output.stdout,
                stderr: output.stderr,
            };
            Ok(crate::extensions::RunnerAdapter::parse(self, input)
                .await?
                .map(RunReport::from))
        })
    }
}
