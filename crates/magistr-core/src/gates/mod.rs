//! Which gates a project runs, resolved from its configuration.
//!
//! One resolver behind every consumer that asks "what does `magistr_gate
//! all` run?": the MCP tool, project validation, and the verification
//! evidence a chunk must present to complete. The loop driver deliberately
//! reads the top-level `quality_gate` instead (see the loop's gates).

mod gate_spec;
mod resolve_gates;

pub use gate_spec::GateSpec;
pub use resolve_gates::resolve_gates;
