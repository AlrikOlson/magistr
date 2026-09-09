//! One quality gate on the status strip.

use crate::job::JobId;
use crate::theme::Icons;

/// A gate the loop driver started as a job: its name, the job's id — the
/// result comes back as [`Msg::Job`](super::Msg::Job) with that id — and
/// its outcome once the job returned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GateLed {
    name: String,
    job: JobId,
    passed: Option<bool>,
}

impl GateLed {
    /// The gate `name`, running as `job`.
    #[must_use]
    pub fn new(name: impl Into<String>, job: JobId) -> Self {
        Self {
            name: name.into(),
            job,
            passed: None,
        }
    }

    /// The gate's name (`check`, `test`, `lint`, …).
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The job running it.
    #[must_use]
    pub const fn job(&self) -> JobId {
        self.job
    }

    /// Whether it passed — `None` while the job runs.
    #[must_use]
    pub const fn passed(&self) -> Option<bool> {
        self.passed
    }

    /// Record the outcome.
    pub(super) fn settle(&mut self, passed: bool) {
        self.passed = Some(passed);
    }

    /// Put this gate back to running as `job` — the loop re-ran it to tell
    /// a flake from a failure, and the strip shows one LED per gate, not
    /// one per attempt.
    pub(super) fn restart(&mut self, job: JobId) {
        self.job = job;
        self.passed = None;
    }

    /// Waiting, passed or failed, in `icons`' vocabulary.
    #[must_use]
    pub const fn glyph(&self, icons: &Icons) -> &'static str {
        icons.gate(self.passed)
    }

    /// The glyph and the name: `✓ check`.
    #[must_use]
    pub fn text(&self, icons: &Icons) -> String {
        format!("{} {}", self.glyph(icons), self.name)
    }
}

#[cfg(test)]
#[path = "gate_led.tests.rs"]
mod tests;
