//! Which gates a project runs, resolved from its configuration.
//!
//! One resolver behind every consumer that asks "what does `magistr_gate
//! all` run?": the MCP tool, project validation, the verification
//! evidence a chunk must present to complete, and — through that evidence
//! capability's `required_gates` — the gates the loop driver runs between
//! iterations, so the loop's receipts are the contracts completion checks.

mod gate_spec;
mod resolve_gates;

pub use gate_spec::GateSpec;
pub use resolve_gates::resolve_gates;
