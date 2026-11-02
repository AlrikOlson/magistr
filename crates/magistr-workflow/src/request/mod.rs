//! Request domain and the `request_*` tool family — the local surface of the
//! stakeholder-request system. A request is a question / suggestion /
//! concern / bug / feedback raised about the project; the local store is a
//! CACHE of the cloud system-of-record (the wire contract is
//! `contract/request-envelope.schema.json`; the cloud client syncs against
//! it). Additive: a fourth family beside `decision_*`, `run_*`, and
//! `plan_*`, mirroring the plan module's domain → engine → mcp shape.

pub mod application;
pub mod broadcast;
pub mod domain;
pub mod engine;
pub mod mcp;
pub mod output_schemas;

pub use engine::RequestEngine;
pub use mcp::RequestService;
