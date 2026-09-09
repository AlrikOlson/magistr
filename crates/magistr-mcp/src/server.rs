//! SDK MCP Server Implementation
//!
//! Provides an in-process MCP server for registering and invoking custom tools.

use std::collections::HashMap;

use super::protocol::{JsonRpcRequest, JsonRpcResponse, McpError};
use super::tool::SdkMcpTool;
use crate::tool_error::{Result, ToolError};

/// An in-process MCP server that hosts custom tools
///
/// The SDK MCP server allows you to register custom tools that Claude can invoke
/// directly without subprocess management. Tools are registered via the builder
/// pattern and invoked through JSONRPC.
///
/// # Example
///
/// ```rust
/// use magistr_mcp::{SdkMcpServer, SdkMcpTool, ToolResult};
/// use serde_json::json;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let server = SdkMcpServer::new("calculator")
///     .version("1.0.0")
///     .tool(SdkMcpTool::new(
///         "add",
///         "Add two numbers",
///         json!({
///             "type": "object",
///             "properties": {
///                 "a": {"type": "number"},
///                 "b": {"type": "number"}
///             }
///         }),
///         |input| Box::pin(async move {
///             let sum = input["a"].as_f64().unwrap_or(0.0)
///                     + input["b"].as_f64().unwrap_or(0.0);
///             Ok(ToolResult::text(format!("Sum: {}", sum)))
///         }),
///     ));
///
/// // Server can now handle MCP requests
/// # Ok(())
/// # }
/// ```
pub struct SdkMcpServer {
    /// Server name
    name: String,
    /// Server version
    version: String,
    /// Registered tools (keyed by tool name)
    tools: HashMap<String, SdkMcpTool>,
}

impl SdkMcpServer {
    /// Create a new MCP server with the given name
    ///
    /// # Example
    ///
    /// ```rust
    /// use magistr_mcp::SdkMcpServer;
    ///
    /// let server = SdkMcpServer::new("my-tools");
    /// ```
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: "1.0.0".to_string(),
            tools: HashMap::new(),
        }
    }

    /// Set the server version
    ///
    /// # Example
    ///
    /// ```rust
    /// use magistr_mcp::SdkMcpServer;
    ///
    /// let server = SdkMcpServer::new("my-tools")
    ///     .version("2.0.0");
    /// ```
    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    /// Register a tool with the server
    ///
    /// Tool names must be unique within a server. If a tool with the same name
    /// already exists, it will be replaced.
    ///
    /// # Example
    ///
    /// ```rust
    /// use magistr_mcp::{SdkMcpServer, SdkMcpTool, ToolResult};
    /// use serde_json::json;
    ///
    /// let server = SdkMcpServer::new("my-tools")
    ///     .tool(SdkMcpTool::new(
    ///         "greet",
    ///         "Greet someone",
    ///         json!({"type": "object"}),
    ///         |_| Box::pin(async { Ok(ToolResult::text("Hello!")) }),
    ///     ));
    /// ```
    pub fn tool(mut self, tool: SdkMcpTool) -> Self {
        let name = tool.name().to_string();
        self.tools.insert(name, tool);
        self
    }

    /// Register multiple tools at once
    ///
    /// # Example
    ///
    /// ```rust
    /// use magistr_mcp::{SdkMcpServer, SdkMcpTool, ToolResult};
    /// use serde_json::json;
    ///
    /// let tools = vec![
    ///     SdkMcpTool::new("tool1", "First tool", json!({}),
    ///         |_| Box::pin(async { Ok(ToolResult::text("1")) })),
    ///     SdkMcpTool::new("tool2", "Second tool", json!({}),
    ///         |_| Box::pin(async { Ok(ToolResult::text("2")) })),
    /// ];
    ///
    /// let server = SdkMcpServer::new("my-tools").tools(tools);
    /// ```
    pub fn tools(mut self, tools: Vec<SdkMcpTool>) -> Self {
        for tool in tools {
            let name = tool.name().to_string();
            self.tools.insert(name, tool);
        }
        self
    }

    /// Get the server name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the server version
    pub fn server_version(&self) -> &str {
        &self.version
    }

    /// Get a tool by name
    pub fn get_tool(&self, name: &str) -> Option<&SdkMcpTool> {
        self.tools.get(name)
    }

    /// Get all registered tools
    pub fn list_tools(&self) -> Vec<&SdkMcpTool> {
        self.tools.values().collect()
    }

    /// Handle an MCP JSONRPC request
    ///
    /// Processes JSONRPC requests according to the MCP protocol:
    /// - `tools/list` - Returns list of available tools
    /// - `tools/call` - Invokes a specific tool
    ///
    /// # Errors
    ///
    /// Returns [`ToolError::Mcp`] if the request is invalid or tool invocation fails.
    pub async fn handle_request(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse> {
        let request_id = request.id.clone().unwrap_or(serde_json::json!(null));

        match request.method.as_str() {
            "tools/list" => self.handle_tools_list(request_id).await,
            "tools/call" => self.handle_tools_call(request_id, request.params).await,
            _ => Ok(JsonRpcResponse::error(
                request_id,
                McpError::method_not_found(&request.method),
            )),
        }
    }

    /// Handle `tools/list` request
    async fn handle_tools_list(&self, request_id: serde_json::Value) -> Result<JsonRpcResponse> {
        let tools: Vec<_> = self
            .tools
            .values()
            .map(|tool| tool.to_tool_info())
            .collect();

        Ok(JsonRpcResponse::success(
            request_id,
            serde_json::json!({
                "tools": tools
            }),
        ))
    }

    /// Handle `tools/call` request
    async fn handle_tools_call(
        &self,
        request_id: serde_json::Value,
        params: Option<serde_json::Value>,
    ) -> Result<JsonRpcResponse> {
        // Validate parameters
        let params = match params {
            Some(p) => p,
            None => {
                return Ok(JsonRpcResponse::error(
                    request_id,
                    McpError::invalid_params("tools/call requires parameters".to_string()),
                ));
            }
        };

        let tool_name = match params["name"].as_str() {
            Some(name) => name,
            None => {
                return Ok(JsonRpcResponse::error(
                    request_id,
                    McpError::invalid_params("Missing tool name in parameters".to_string()),
                ));
            }
        };

        // Check if tool exists
        let tool = match self.tools.get(tool_name) {
            Some(t) => t,
            None => {
                return Ok(JsonRpcResponse::error(
                    request_id,
                    McpError::tool_not_found(tool_name),
                ));
            }
        };

        let arguments = params["arguments"].clone();

        // Validate arguments against the tool's declared JSON Schema
        if let Err(msg) = validate_tool_arguments(&arguments, tool.input_schema()) {
            return Ok(JsonRpcResponse::error(
                request_id,
                McpError::invalid_params(msg),
            ));
        }

        // Invoke the tool
        match tool.invoke(arguments).await {
            Ok(result) => {
                let result_json = serde_json::to_value(result)
                    .map_err(|e| ToolError::Mcp(format!("Failed to serialize result: {e}")))?;

                Ok(JsonRpcResponse::success(request_id, result_json))
            }
            Err(e) => Ok(JsonRpcResponse::error(
                request_id,
                McpError::internal_error(format!("Tool execution failed: {e}")),
            )),
        }
    }
}

/// Validate tool arguments against a JSON Schema.
///
/// Returns `Ok(())` if the schema is empty/trivial or the arguments pass validation.
/// Returns `Err(message)` with a human-readable description of which fields failed.
fn validate_tool_arguments(
    arguments: &serde_json::Value,
    schema: &serde_json::Value,
) -> std::result::Result<(), String> {
    // Skip validation for empty or trivial schemas
    if schema.is_null()
        || *schema == serde_json::json!({})
        || *schema == serde_json::json!({"type": "object"})
    {
        return Ok(());
    }

    let validator =
        jsonschema::validator_for(schema).map_err(|e| format!("Invalid tool schema: {e}"))?;

    if validator.is_valid(arguments) {
        return Ok(());
    }

    let messages: Vec<String> = validator
        .iter_errors(arguments)
        .map(|e| {
            let path = e.instance_path.to_string();
            if path.is_empty() {
                e.to_string()
            } else {
                format!("{path}: {e}")
            }
        })
        .collect();

    Err(format!("Schema validation failed: {}", messages.join("; ")))
}

impl std::fmt::Debug for SdkMcpServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SdkMcpServer")
            .field("name", &self.name)
            .field("version", &self.version)
            .field("tools", &self.tools.keys().collect::<Vec<_>>())
            .finish()
    }
}

#[cfg(test)]
#[path = "server.tests.rs"]
mod tests;
