use super::*;

#[test]
fn text_block_debug() {
    let block = ContentBlock::Text {
        text: "hello".into(),
    };
    let dbg = format!("{block:?}");
    assert!(dbg.contains("hello"));
}

#[test]
fn tool_use_block() {
    let block = ContentBlock::ToolUse {
        id: "tu-1".into(),
        name: "Write".into(),
        input: serde_json::json!({"path": "foo.rs"}),
        kind: Some(ToolKind::Edit),
        locations: vec![],
        tool_name: Some("Write".into()),
    };
    if let ContentBlock::ToolUse { id, kind, .. } = &block {
        assert_eq!(id, "tu-1");
        assert_eq!(*kind, Some(ToolKind::Edit));
    } else {
        panic!("expected ToolUse");
    }
}

#[test]
fn tool_result_block() {
    let block = ContentBlock::ToolResult {
        tool_use_id: "tu-1".into(),
        content: Some("ok".into()),
        is_error: Some(false),
    };
    if let ContentBlock::ToolResult { is_error, .. } = &block {
        assert_eq!(*is_error, Some(false));
    } else {
        panic!("expected ToolResult");
    }
}

#[test]
fn image_block() {
    let block = ContentBlock::Image {
        mime_type: "image/png".into(),
        uri: Some("file:///tmp/img.png".into()),
        alt: "screenshot".into(),
    };
    if let ContentBlock::Image { alt, .. } = &block {
        assert_eq!(alt, "screenshot");
    } else {
        panic!("expected Image");
    }
}

#[test]
fn assistant_content() {
    let content = AssistantContent {
        model: "claude-sonnet-4-20250514".into(),
        content: vec![ContentBlock::Text { text: "hi".into() }],
    };
    assert_eq!(content.model, "claude-sonnet-4-20250514");
    assert_eq!(content.content.len(), 1);
}

#[test]
fn user_body_string() {
    let body = UserBody::String("prompt".into());
    if let UserBody::String(s) = &body {
        assert_eq!(s, "prompt");
    } else {
        panic!("expected String");
    }
}

#[test]
fn user_content_with_none() {
    let uc = UserContent {
        role: "user".into(),
        content: None,
    };
    assert!(uc.content.is_none());
}
