/// One captured command run handed to a runner provider: the argv line it
/// may claim, the exit code and both output streams.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunnerInput {
    pub command: String,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}
