//! The iteration loop on the cockpit (`crates/magistr-desktop`): `run_loop`'s
//! control flow ported off the frozen legacy tree.
//!
//! [`LoopDriver`] is a task beside the cockpit's runtime. It owns the
//! [`IterationLoop`](crate::workflow::iteration::IterationLoop) and, per
//! iteration, builds the prompt, connects an agent through a
//! [`Connector`], runs the session producer over a tapped inbox (so it
//! sees the turn end exactly as the cockpit does), runs the quality gates
//! as jobs on the blocking pool — never on the loop thread — and reports
//! every step as a [`LoopEvent`](magistr_cockpit::cockpit::LoopEvent) for the
//! status strip. The keys the cockpit turns into
//! [`LoopCmd`](magistr_cockpit::cockpit::LoopCmd)s reach it through the
//! runtime's commands; everything else the cockpit asks (prompts,
//! cancels, permission answers) is forwarded to the live session.
//!
//! `update` and `view` never see any of this: they fold messages. The
//! driver owns every effect.

mod checkpoint_detach;
mod connector;
mod control;
mod driver;
mod failure_brief;
mod gate_job;
mod gate_outcome;
mod iteration_metrics;
mod loop_end;
mod loop_report;

pub use connector::Connector;
pub use driver::LoopDriver;
pub(crate) use gate_outcome::GateOutcome;
pub use iteration_metrics::IterationMetrics;
pub use loop_end::LoopEnd;
pub use loop_report::LoopReport;
