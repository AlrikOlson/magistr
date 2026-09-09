use super::*;
use crate::ToolResult;
use serde_json::json;

fn create_test_tool(name: &str) -> SdkMcpTool {
    SdkMcpTool::new(
        name,
        format!("Test tool {name}"),
        json!({"type": "object"}),
        |input| {
            Box::pin(async move {
                let text = input["text"].as_str().unwrap_or("default");
                Ok(ToolResult::text(text))
            })
        },
    )
}

#[test]
fn test_server_creation() {
    let server = SdkMcpServer::new("test-server").version("1.0.0");

    assert_eq!(server.name(), "test-server");
    assert_eq!(server.server_version(), "1.0.0");
}

#[test]
fn test_tool_registration() {
    let server = SdkMcpServer::new("test")
        .tool(create_test_tool("tool1"))
        .tool(create_test_tool("tool2"));

    assert_eq!(server.list_tools().len(), 2);
    assert!(server.get_tool("tool1").is_some());
    assert!(server.get_tool("tool2").is_some());
    assert!(server.get_tool("tool3").is_none());
}

#[test]
fn test_multiple_tools_registration() {
    let tools = vec![create_test_tool("a"), create_test_tool("b")];

    let server = SdkMcpServer::new("test").tools(tools);

    assert_eq!(server.list_tools().len(), 2);
}

#[tokio::test]
async fn test_tools_list_request() {
    let server = SdkMcpServer::new("test")
        .tool(create_test_tool("tool1"))
        .tool(create_test_tool("tool2"));

    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(1)),
        method: "tools/list".to_string(),
        params: None,
    };

    let response = server.handle_request(request).await.unwrap();

    assert!(response.result.is_some());
    let result = response.result.unwrap();
    let tools = result["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 2);
}

#[tokio::test]
async fn test_tools_call_request() {
    let server = SdkMcpServer::new("test").tool(create_test_tool("echo"));

    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(1)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "echo",
            "arguments": {
                "text": "hello"
            }
        })),
    };

    let response = server.handle_request(request).await.unwrap();

    assert!(response.result.is_some());
    let result = response.result.unwrap();
    assert_eq!(result["content"][0]["text"], "hello");
}

#[tokio::test]
async fn test_unknown_method() {
    let server = SdkMcpServer::new("test");

    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(1)),
        method: "unknown/method".to_string(),
        params: None,
    };

    let response = server.handle_request(request).await.unwrap();

    assert!(response.error.is_some());
    assert_eq!(response.error.unwrap().code, -32601);
}

#[tokio::test]
async fn test_tool_not_found() {
    let server = SdkMcpServer::new("test");

    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(1)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "nonexistent",
            "arguments": {}
        })),
    };

    let response = server.handle_request(request).await.unwrap();

    assert!(response.error.is_some());
}

// --- Schema validation tests ---

fn create_strict_tool(name: &str) -> SdkMcpTool {
    SdkMcpTool::new(
        name,
        format!("Strict tool {name}"),
        json!({
            "type": "object",
            "properties": {
                "a": {"type": "number"},
                "b": {"type": "number"}
            },
            "required": ["a", "b"]
        }),
        |input| {
            Box::pin(async move {
                let a = input["a"].as_f64().unwrap_or(0.0);
                let b = input["b"].as_f64().unwrap_or(0.0);
                Ok(ToolResult::text(format!("{}", a + b)))
            })
        },
    )
}

#[test]
fn validate_tool_arguments_valid() {
    let schema = json!({
        "type": "object",
        "properties": {
            "a": {"type": "number"},
            "b": {"type": "number"}
        },
        "required": ["a", "b"]
    });
    let args = json!({"a": 1, "b": 2});
    assert!(validate_tool_arguments(&args, &schema).is_ok());
}

#[test]
fn validate_tool_arguments_missing_required() {
    let schema = json!({
        "type": "object",
        "properties": {
            "a": {"type": "number"},
            "b": {"type": "number"}
        },
        "required": ["a", "b"]
    });
    let args = json!({"a": 1});
    let err = validate_tool_arguments(&args, &schema).unwrap_err();
    assert!(err.contains("Schema validation failed"), "got: {err}");
    assert!(
        err.contains("b"),
        "error should mention missing field 'b', got: {err}"
    );
}

#[test]
fn validate_tool_arguments_wrong_type() {
    let schema = json!({
        "type": "object",
        "properties": {
            "a": {"type": "number"}
        },
        "required": ["a"]
    });
    let args = json!({"a": "not a number"});
    let err = validate_tool_arguments(&args, &schema).unwrap_err();
    assert!(err.contains("Schema validation failed"), "got: {err}");
}

#[test]
fn validate_tool_arguments_skips_empty_schema() {
    // Empty / trivial schemas should not reject anything
    assert!(validate_tool_arguments(&json!({}), &json!({})).is_ok());
    assert!(validate_tool_arguments(&json!({}), &json!({"type": "object"})).is_ok());
    assert!(validate_tool_arguments(&json!({}), &serde_json::Value::Null).is_ok());
}

#[tokio::test]
async fn schema_validation_rejects_invalid_call() {
    let server = SdkMcpServer::new("test").tool(create_strict_tool("add"));

    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(1)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "add",
            "arguments": {"a": 1}
        })),
    };

    let response = server.handle_request(request).await.unwrap();
    assert!(response.error.is_some());
    let error = response.error.unwrap();
    assert_eq!(error.code, -32602); // Invalid params
    assert!(
        error.message.contains("Schema validation failed"),
        "got: {}",
        error.message
    );
}

#[tokio::test]
async fn schema_validation_passes_valid_call() {
    let server = SdkMcpServer::new("test").tool(create_strict_tool("add"));

    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(1)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "add",
            "arguments": {"a": 1, "b": 2}
        })),
    };

    let response = server.handle_request(request).await.unwrap();
    assert!(response.result.is_some());
    let result = response.result.unwrap();
    assert_eq!(result["content"][0]["text"], "3");
}
