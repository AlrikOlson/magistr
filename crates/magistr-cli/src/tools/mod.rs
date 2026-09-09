//! Tool-call infrastructure for magistr.
//!
//! Provides a clean, typed layer over ACP tool call events. Instead of
//! inspecting raw `serde_json::Value` everywhere, consumers register
//! interest in specific tools and receive typed Rust structs.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────┐
//! │   ToolRegistry      │  ← register tools by name
//! │   ├─ magistr_recommend  │
//! │   ├─ magistr_configure  │  (future)
//! │   └─ ...            │
//! └─────────┬───────────┘
//!           │ .try_extract(name, input)
//!           ▼
//! ┌─────────────────────┐
//! │  ToolOutput enum    │  ← typed, deserialized payloads
//! │   ├─ Recommend(..)  │
//! │   └─ Unknown        │
//! └─────────────────────┘
//! ```
//!
//! The registry also generates the MCP server configuration needed to
//! make these tools available to agent sessions (via `.mcp.json`).

pub mod classify;
pub mod gate_patterns;
pub mod normalize;
pub mod patterns;

mod registry;
mod server;

pub use registry::{Choice, MagistrTool, Question, ToolOutput, ToolRegistry};
pub use server::{is_workbench_available, write_interview_mcp_config, write_mcp_config};
