//! Plan domain and the `plan_*` tool family — the long-horizon
//! plan-of-plans that sits above `run_*` goals and links to `think_*`
//! reasoning. The foundation is the domain + engine + persistence; the rmcp
//! wire adapter (`mcp::PlanService`) puts `plan_*` on the live MCP wire.

pub mod application;
pub mod broadcast;
pub mod domain;
pub mod engine;
pub mod import;
pub mod mcp;
pub mod name;
pub mod output_schemas;
pub mod region;

pub use engine::PlanEngine;
pub use mcp::PlanService;
