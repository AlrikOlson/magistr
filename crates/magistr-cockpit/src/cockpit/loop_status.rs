//! Where the iteration loop stands.

use std::time::Duration;

use super::gate_led::GateLed;
use super::loop_stage::LoopStage;
use super::loop_verdict::LoopVerdict;
use crate::job::JobId;
use crate::theme::Icons;

/// The iteration loop as the status strip shows it: which iteration of
/// how many, the gates it ran and their outcomes, how long it has been
/// running, where the driver stands within the iteration (its
/// [`LoopStage`]), whether the user paused it, why it ended once it
/// did, and what the judge has said — how many verdicts the loop has taken and
/// the last one.
///
/// The loop driver reports every change as a
/// [`LoopEvent`](super::LoopEvent); the elapsed time is what the driver
/// last said, since `update` has no clock. The verdict count is the
/// *loop's*, carried across iterations, so a strip on iteration 4 says
/// three verdicts have already gone by.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoopStatus {
    iteration: u32,
    max: u32,
    gates: Vec<GateLed>,
    stage: LoopStage,
    elapsed: Duration,
    paused: bool,
    paused_at: Option<Duration>,
    ended: Option<String>,
    verdicts: u32,
    verdict: Option<LoopVerdict>,
}

impl LoopStatus {
    /// Iteration `iteration` of `max`, just started.
    #[must_use]
    pub const fn new(iteration: u32, max: u32) -> Self {
        Self {
            iteration,
            max,
            gates: Vec::new(),
            stage: LoopStage::Agent,
            elapsed: Duration::ZERO,
            paused: false,
            paused_at: None,
            ended: None,
            verdicts: 0,
            verdict: None,
        }
    }

    /// This status with the verdicts an earlier iteration's status
    /// counted — how a new iteration keeps the loop's count.
    #[must_use]
    pub fn carrying(mut self, earlier: Option<&Self>) -> Self {
        if let Some(earlier) = earlier {
            self.verdicts = earlier.verdicts;
            self.verdict.clone_from(&earlier.verdict);
        }
        self
    }

    /// How many verdicts the judge has given this loop.
    #[must_use]
    pub const fn verdicts(&self) -> u32 {
        self.verdicts
    }

    /// The last verdict, once there is one.
    #[must_use]
    pub const fn verdict(&self) -> Option<&LoopVerdict> {
        self.verdict.as_ref()
    }

    /// The strip's judge LED — `✓ judge`, `✗ judge` — once a verdict
    /// exists; nothing before, so a loop without a judge is the strip it
    /// always was.
    #[must_use]
    pub fn verdict_text(&self, icons: &Icons) -> Option<String> {
        self.verdict
            .as_ref()
            .map(|verdict| format!("{} judge", verdict.glyph(icons)))
    }

    /// The iteration running (1-based).
    #[must_use]
    pub const fn iteration(&self) -> u32 {
        self.iteration
    }

    /// How many iterations the loop may run.
    #[must_use]
    pub const fn max(&self) -> u32 {
        self.max
    }

    /// `iteration i/N`.
    #[must_use]
    pub fn iteration_text(&self) -> String {
        format!("iteration {}/{}", self.iteration, self.max)
    }

    /// Where the driver stands within the iteration, as it last said.
    #[must_use]
    pub const fn stage(&self) -> LoopStage {
        self.stage
    }

    /// The gates started this iteration, in the order they started.
    #[must_use]
    pub fn gates(&self) -> &[GateLed] {
        &self.gates
    }

    /// Whether every gate passed — `None` until every gate has returned,
    /// or when none ran.
    #[must_use]
    pub fn gates_passed(&self) -> Option<bool> {
        if self.gates.is_empty() {
            return None;
        }
        self.gates
            .iter()
            .try_fold(true, |all, gate| gate.passed().map(|p| all && p))
    }

    /// How long the iteration has been running, as last reported.
    #[must_use]
    pub const fn elapsed(&self) -> Duration {
        self.elapsed
    }

    /// The elapsed time in words: `45s`, `1m 23s`, `1h 02m 05s`.
    #[must_use]
    pub fn elapsed_text(&self) -> String {
        let secs = self.elapsed.as_secs();
        let (hours, minutes, seconds) = (secs / 3600, (secs % 3600) / 60, secs % 60);
        if hours > 0 {
            format!("{hours}h {minutes:02}m {seconds:02}s")
        } else if minutes > 0 {
            format!("{minutes}m {seconds:02}s")
        } else {
            format!("{seconds}s")
        }
    }

    /// Whether the user paused the loop (it stops at the next boundary).
    #[must_use]
    pub const fn is_paused(&self) -> bool {
        self.paused
    }

    /// Why the loop ended, once it did.
    /// How long the loop has been paused on its own clock — the
    /// elapsed it has reported since the pause. `None` while running.
    #[must_use]
    pub fn pause_age(&self) -> Option<Duration> {
        self.elapsed.checked_sub(self.paused_at?)
    }

    #[must_use]
    pub fn ended(&self) -> Option<&str> {
        self.ended.as_deref()
    }

    pub(super) fn set_elapsed(&mut self, elapsed: Duration) {
        self.elapsed = elapsed;
    }

    pub(super) fn set_stage(&mut self, stage: LoopStage) {
        self.stage = stage;
    }

    pub(super) fn start_gate(&mut self, name: impl Into<String>, job: JobId) {
        let name = name.into();
        // One LED per gate, not per attempt: a gate the loop re-ran to
        // tell a flake from a failure goes back to waiting in the slot it
        // already has, rather than appearing on the strip a second time.
        match self.gates.iter_mut().find(|gate| gate.name() == name) {
            Some(gate) => gate.restart(job),
            None => self.gates.push(GateLed::new(name, job)),
        }
    }

    /// Record `job`'s outcome on its gate; `false` when no gate runs as
    /// `job`.
    pub(super) fn settle_gate(&mut self, job: JobId, passed: bool) -> bool {
        match self.gates.iter_mut().find(|gate| gate.job() == job) {
            Some(gate) => {
                gate.settle(passed);
                true
            }
            None => false,
        }
    }

    pub(super) fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
        // The pause is stamped on the loop's own clock, so its age is
        // the elapsed the driver reports past it — never a clock read.
        self.paused_at = paused.then_some(self.elapsed);
    }

    pub(super) fn end(&mut self, why: impl Into<String>) {
        self.ended = Some(why.into());
    }

    /// Count one more verdict and keep it as the last.
    pub(super) fn record_verdict(&mut self, verdict: LoopVerdict) {
        self.verdicts = self.verdicts.saturating_add(1);
        self.verdict = Some(verdict);
    }
}

#[cfg(test)]
#[path = "loop_status.tests.rs"]
mod tests;
