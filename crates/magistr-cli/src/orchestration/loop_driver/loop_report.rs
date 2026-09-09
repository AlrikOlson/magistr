//! What the loop amounted to.

use std::fmt;

use super::iteration_metrics::IterationMetrics;
use super::loop_end::LoopEnd;

/// The loop's report once it ended: how many iterations ran an agent
/// turn to its end, why the loop stopped, and what each iteration cost
/// ([`IterationMetrics`]). The entry point prints it under the
/// cockpit's summary once the terminal is handed back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoopReport {
    iterations: u32,
    end: LoopEnd,
    goal: Option<String>,
    metrics: Vec<IterationMetrics>,
}

impl LoopReport {
    /// `iterations` completed, then `end`.
    #[must_use]
    pub const fn new(iterations: u32, end: LoopEnd) -> Self {
        Self {
            iterations,
            end,
            goal: None,
            metrics: Vec::new(),
        }
    }

    /// What each iteration cost, in order — printed one line per
    /// iteration under the headline.
    #[must_use]
    pub fn with_metrics(mut self, metrics: Vec<IterationMetrics>) -> Self {
        self.metrics = metrics;
        self
    }

    /// What each iteration cost, in order.
    #[must_use]
    pub fn metrics(&self) -> &[IterationMetrics] {
        &self.metrics
    }

    /// Record who drove the goal loop — the loop's own record of the
    /// negotiated outcome (`extension`, `command`, `extension → native
    /// (no echo)`, `native`), printed with the report. `None` (the
    /// default) prints nothing, so a plain loop's report is unchanged.
    #[must_use]
    pub fn with_goal(mut self, goal: Option<String>) -> Self {
        self.goal = goal;
        self
    }

    /// Who drove the goal loop, when one ran.
    #[must_use]
    pub fn goal(&self) -> Option<&str> {
        self.goal.as_deref()
    }

    /// Iterations whose agent turn ran to its end.
    #[must_use]
    pub const fn iterations(&self) -> u32 {
        self.iterations
    }

    /// Why the loop stopped.
    #[must_use]
    pub const fn end(&self) -> &LoopEnd {
        &self.end
    }
}

impl fmt::Display for LoopReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "loop: {} iteration{} · {}",
            self.iterations,
            if self.iterations == 1 { "" } else { "s" },
            self.end
        )?;
        if let Some(goal) = &self.goal {
            write!(f, " · goal: {goal}")?;
        }
        for (i, metrics) in self.metrics.iter().enumerate() {
            write!(f, "\n  iteration {}: {metrics}", i + 1)?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "loop_report.tests.rs"]
mod tests;
