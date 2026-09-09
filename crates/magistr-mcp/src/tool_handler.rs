//! The boxed async function behind every tool.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::tool::ToolResult;
use crate::tool_error::Result;

/// A tool handler: an async function from JSON input to a [`ToolResult`].
///
/// Handlers are `Send + Sync + 'static` so the server can invoke them
/// concurrently from any task.
pub type ToolHandler = Arc<
    dyn Fn(serde_json::Value) -> Pin<Box<dyn Future<Output = Result<ToolResult>> + Send>>
        + Send
        + Sync,
>;
