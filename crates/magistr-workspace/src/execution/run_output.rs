/// One captured execution as the parsers see it (wit: `run-output`): the
/// resolved command line a parser may claim, the exit code and both streams.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOutput {
    pub command: String,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl RunOutput {
    /// Both streams as the retained adapters read them: stdout, a newline,
    /// stderr, split on `\n` with no other normalization.
    #[must_use]
    pub fn combined_lines(&self) -> Vec<String> {
        format!("{}\n{}", self.stdout, self.stderr)
            .split('\n')
            .map(str::to_owned)
            .collect()
    }
}
