use super::*;
use crate::content_block::ContentBlock;

#[test]
fn user_message() {
    let msg = AgentMessage::User {
        parent_tool_use_id: None,
        message: UserContent {
            role: "user".into(),
            content: None,
        },
        session_id: Some(SessionId::new("s1")),
    };
    if let AgentMessage::User { session_id, .. } = &msg {
        assert_eq!(session_id.as_ref().unwrap().as_str(), "s1");
    } else {
        panic!("expected User");
    }
}

#[test]
fn assistant_message() {
    let msg = AgentMessage::Assistant {
        parent_tool_use_id: None,
        message: AssistantContent {
            model: "test".into(),
            content: vec![ContentBlock::Text { text: "hi".into() }],
        },
        session_id: None,
    };
    if let AgentMessage::Assistant { message, .. } = &msg {
        assert_eq!(message.content.len(), 1);
    } else {
        panic!("expected Assistant");
    }
}

#[test]
fn result_message() {
    let msg = AgentMessage::Result {
        subtype: "success".into(),
        duration_ms: 100,
        duration_api_ms: 80,
        is_error: false,
        num_turns: 1,
        session_id: SessionId::new("s1"),
        total_cost_usd: Some(0.01),
        usage: None,
        result: None,
    };
    if let AgentMessage::Result { is_error, .. } = &msg {
        assert!(!is_error);
    } else {
        panic!("expected Result");
    }
}

#[test]
fn plan_message() {
    let msg = AgentMessage::Plan { entries: vec![] };
    if let AgentMessage::Plan { entries } = &msg {
        assert!(entries.is_empty());
    } else {
        panic!("expected Plan");
    }
}

#[test]
fn cancelled_message() {
    let msg = AgentMessage::Cancelled {
        session_id: SessionId::new("s1"),
        reason: "user request".into(),
    };
    if let AgentMessage::Cancelled { reason, .. } = &msg {
        assert_eq!(reason, "user request");
    } else {
        panic!("expected Cancelled");
    }
}

#[test]
fn usage_update() {
    let msg = AgentMessage::UsageUpdate {
        context_tokens_used: 5000,
        context_window_size: 200_000,
        cost_usd: Some(0.05),
    };
    if let AgentMessage::UsageUpdate {
        context_tokens_used,
        ..
    } = &msg
    {
        assert_eq!(*context_tokens_used, 5000);
    } else {
        panic!("expected UsageUpdate");
    }
}
