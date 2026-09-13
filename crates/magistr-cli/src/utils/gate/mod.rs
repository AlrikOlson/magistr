//! One quality-gate runner, behind both callers.
//!
//! `magistr_gate` (the MCP tool and the `mcp-serve` service) and the loop
//! driver's between-iteration gates used to run a gate two different ways:
//! the MCP path captured both streams, parsed failure rows and truncated,
//! while the driver's job kept `{name, passed, duration}` and dropped
//! `stdout`/`stderr` at the source — so the cockpit's whole account of a
//! red gate was one sentence. This module is the single runner both go
//! through, so the two paths cannot disagree about what a gate did.
//!
//! The *resolver* is shared too, one step up: the MCP path resolves its
//! commands through `resolve_gates` (the polyglot, per-archetype specs)
//! and the loop driver asks the evidence capability's `required_gates`,
//! which is `resolve_gates("all")` filtered to the required set — the
//! contracts native completion verifies. The loop used to read the
//! top-level `quality_gate` on its own, which on a polyglot project or one
//! with required custom gates left receipts completion could never accept.

mod gate_run;
mod gate_verdict;
mod nextest_report;
mod run_gate;
mod truncate_output;

pub(crate) use gate_run::GateRun;
pub(crate) use gate_verdict::GateVerdict;
pub(crate) use nextest_report::nextest_junit_report_path;
pub(crate) use run_gate::run_gate;
// The runner reaches the budget through the sibling module; this
// re-export exists for the MCP path's own truncation test.
#[cfg(test)]
pub(crate) use truncate_output::MAX_OUTPUT_BYTES;
