use super::{RunOutput, RunReport};

/// One runner family's output parser: `can_parse` claims a resolved command
/// line, `parse_output` extracts structure. `None` is the honest "could not
/// extract" that makes the run tool fall back to head+tail rather than guess.
/// The bundled parsers are an open, trust-ordered registry
/// ([`super::bundled_runners`]); a composition may prepend or replace them,
/// and third-party ecosystems arrive as [`super::PluginRunnerLane`] guests.
pub trait RunnerParser: Send + Sync {
    fn name(&self) -> &'static str;
    fn can_parse(&self, command_line: &str) -> bool;
    fn parse_output(&self, output: &RunOutput) -> Option<RunReport>;
}
