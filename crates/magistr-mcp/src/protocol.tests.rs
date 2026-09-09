use super::*;
use serde_json::json;

#[test]
fn test_jsonrpc_request_serialization() {
    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(1)),
        method: "tools/list".to_string(),
        params: None,
    };

    let serialized = serde_json::to_string(&request).unwrap();
    assert!(serialized.contains("\"jsonrpc\":\"2.0\""));
    assert!(serialized.contains("\"method\":\"tools/list\""));
}

#[test]
fn test_jsonrpc_success_response() {
    let response = JsonRpcResponse::success(json!(1), json!({"result": "ok"}));

    assert_eq!(response.jsonrpc, "2.0");
    assert_eq!(response.id, json!(1));
    assert!(response.result.is_some());
    assert!(response.error.is_none());
}

#[test]
fn test_jsonrpc_error_response() {
    let error = McpError::method_not_found("unknown/method");
    let response = JsonRpcResponse::error(json!(1), error);

    assert_eq!(response.jsonrpc, "2.0");
    assert_eq!(response.id, json!(1));
    assert!(response.result.is_none());
    assert!(response.error.is_some());

    let err = response.error.unwrap();
    assert_eq!(err.code, -32601);
    assert!(err.message.contains("Method not found"));
}

#[test]
fn test_mcp_error_codes() {
    assert_eq!(McpError::method_not_found("test").code, -32601);
    assert_eq!(McpError::invalid_params("test".to_string()).code, -32602);
    assert_eq!(McpError::internal_error("test".to_string()).code, -32603);
    assert_eq!(McpError::tool_not_found("test").code, -32001);
}
