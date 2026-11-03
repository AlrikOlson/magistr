//! Which checks a project runs, resolved from its configuration.
//!
//! One resolver behind every consumer that asks "what does `magistr_check
//! all` run?": the MCP tool, project validation, the verification
//! evidence a task must present to complete, and — through that evidence
//! capability's `required_checks` — the checks the loop driver runs between
//! iterations, so the loop's receipts are the contracts completion checks.

mod execution_policy;
pub use execution_policy::{configured_policy, execution_policy, verification_policy};
mod check_spec;
mod resolve_checks;

pub use check_spec::CheckSpec;
pub use resolve_checks::{clippy_covers, consolidate_checks, resolve_checks};
