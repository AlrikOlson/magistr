//! What one iteration cost.

use std::fmt;
use std::time::Duration;

/// One iteration's measure: how long each phase took, and what the turn
/// put in the agent's context window. Recorded by the driver as the
/// iteration runs and printed under the loop's report, so the numbers
/// the goal loop is judged on are taken by the loop itself
/// rather than read off a screen.
///
/// `tokens` is the highest `used` level the agent reported during the
/// turn — ACP's `UsageUpdate.used` is "tokens currently in context", a
/// level, and every iteration runs on a fresh session, so the level at
/// the turn's end is what that turn put in context. `None` when the
/// agent never reported usage. `judge` is `None` when no judge was
/// asked (`MAGISTR_GOAL=off`, a delegated iteration, nothing to judge).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IterationMetrics {
    /// The agent's turn, connection included.
    pub turn: Duration,
    /// The quality gates, all of them, as they ran.
    pub gates: Duration,
    /// The judge's verdict, connection included, when one was asked.
    pub judge: Option<Duration>,
    /// Tokens in the agent's context at the end of the turn.
    pub tokens: Option<u64>,
}

impl fmt::Display for IterationMetrics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "turn {} · gates {}", clock(self.turn), clock(self.gates))?;
        if let Some(judge) = self.judge {
            write!(f, " · judge {}", clock(judge))?;
        }
        match self.tokens {
            Some(tokens) => write!(f, " · {} tokens in context", count(tokens)),
            None => write!(f, " · usage not reported"),
        }
    }
}

/// `12.3s` under a minute, `4m10s` from there.
fn clock(duration: Duration) -> String {
    let secs = duration.as_secs_f64();
    if secs < 60.0 {
        format!("{secs:.1}s")
    } else {
        let minutes = (secs / 60.0).floor();
        let rest = secs - minutes * 60.0;
        format!("{minutes:.0}m{rest:02.0}s")
    }
}

/// `987`, `12.3k`, `1.2M`.
#[allow(clippy::cast_precision_loss)] // a display figure, rounded to a decimal anyway
fn count(n: u64) -> String {
    if n < 1_000 {
        n.to_string()
    } else if n < 1_000_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    }
}

#[cfg(test)]
#[path = "iteration_metrics.tests.rs"]
mod tests;
