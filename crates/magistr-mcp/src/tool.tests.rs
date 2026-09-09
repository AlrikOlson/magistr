use super::*;
use serde_json::json;

#[tokio::test]
async fn test_tool_creation() {
    let tool = SdkMcpTool::new(
        "test",
        "Test tool",
        json!({"type": "object"}),
        |_input| async { Ok(ToolResult::text("test")) },
    );

    assert_eq!(tool.name(), "test");
    assert_eq!(tool.description(), "Test tool");
}

#[tokio::test]
async fn test_tool_invocation() {
    let tool = SdkMcpTool::new(
        "echo",
        "Echo tool",
        json!({"type": "object"}),
        |input| async move {
            let text = input["text"].as_str().unwrap_or("empty");
            Ok(ToolResult::text(text))
        },
    );

    let result = tool.invoke(json!({"text": "hello"})).await.unwrap();
    assert_eq!(result.content.len(), 1);

    if let ToolContent::Text { text } = &result.content[0] {
        assert_eq!(text, "hello");
    } else {
        panic!("Expected text content");
    }
}

#[test]
fn test_tool_result_text() {
    let result = ToolResult::text("success");
    assert_eq!(result.content.len(), 1);
    assert!(result.is_error.is_none());
}

#[test]
fn test_tool_result_error() {
    let result = ToolResult::error("failed");
    assert_eq!(result.content.len(), 1);
    assert_eq!(result.is_error, Some(true));
}

#[test]
fn test_tool_content_serialization() {
    let content = ToolContent::Text {
        text: "test".to_string(),
    };
    let json = serde_json::to_string(&content).unwrap();
    assert!(json.contains("\"type\":\"text\""));
    assert!(json.contains("\"text\":\"test\""));
}

#[test]
fn test_tool_info() {
    let tool = SdkMcpTool::new("test", "Test tool", json!({"type": "object"}), |_| async {
        Ok(ToolResult::text("test"))
    });

    let info = tool.to_tool_info();
    assert_eq!(info["name"], "test");
    assert_eq!(info["description"], "Test tool");
    assert!(info["inputSchema"].is_object());
}
