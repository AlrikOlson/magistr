//! In-process MCP tool server.
//!
//! Hosts the `magistr_*` tools that `magistr mcp` mounts beside the
//! think-and-ship families: a tool is a name, a description, a JSON Schema and
//! an async handler ([`SdkMcpTool`]); a server ([`SdkMcpServer`]) keeps them by
//! name, answers `tools/list`, and validates `tools/call` arguments against
//! the declared schema before dispatching.
//!
//! This crate speaks plain MCP JSON-RPC ([`JsonRpcRequest`] /
//! [`JsonRpcResponse`]) and knows nothing about ACP — the rmcp service in
//! `magistr-cli` drives it with the real request context.
//!
//! ```rust
//! use magistr_mcp::{SdkMcpServer, SdkMcpTool, ToolResult};
//! use serde_json::json;
//!
//! let greet = SdkMcpTool::new(
//!     "greet",
//!     "Greet a user by name",
//!     json!({"type": "object", "properties": {"name": {"type": "string"}}, "required": ["name"]}),
//!     |input| Box::pin(async move {
//!         let name = input["name"].as_str().unwrap_or("stranger");
//!         Ok(ToolResult::text(format!("Hello, {name}!")))
//!     }),
//! );
//! let server = SdkMcpServer::new("my-tools").version("1.0.0").tool(greet);
//! assert!(server.get_tool("greet").is_some());
//! ```

#![warn(missing_docs)]

pub mod protocol;
mod server;
mod tool;
mod tool_error;
mod tool_handler;

pub use protocol::{JsonRpcRequest, JsonRpcResponse, McpError};
pub use server::SdkMcpServer;
pub use tool::{SdkMcpTool, ToolContent, ToolResult};
pub use tool_error::{Result, ToolError};
pub use tool_handler::ToolHandler;
