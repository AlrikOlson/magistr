//! One check runner, behind both callers.
//!
//! `magistr_check` (the MCP tool and the `mcp` service) and the loop
//! driver's between-iteration checks used to run a check two different ways:
//! the MCP path captured both streams, parsed failure rows and truncated,
//! while the driver's job kept `{name, passed, duration}` and dropped
//! `stdout`/`stderr` at the source — so the cockpit's whole account of a
//! red check was one sentence. This module is the single runner both go
//! through, so the two paths cannot disagree about what a check did.
//!
//! The *resolver* is shared too, one step up: the MCP path resolves its
//! commands through `resolve_checks` (the polyglot, per-archetype specs)
//! and the loop driver asks the evidence capability's `required_checks`,
//! which is `resolve_checks("all")` filtered to the required set — the
//! contracts native completion verifies. The loop used to read the
//! top-level `checks` on its own, which on a polyglot project or one
//! with required custom checks left receipts completion could never accept.

mod check_run;
mod check_verdict;
mod nextest_report;
mod run_check;
mod truncate_output;

pub(crate) use check_run::CheckRun;
#[cfg(test)]
pub(crate) use check_verdict::CheckVerdict;
pub(crate) use nextest_report::nextest_junit_report_path;
pub(crate) use run_check::run_check;
// The runner reaches the budget through the sibling module; this
// re-export exists for the MCP path's own truncation test.
#[cfg(test)]
pub(crate) use truncate_output::MAX_OUTPUT_BYTES;
