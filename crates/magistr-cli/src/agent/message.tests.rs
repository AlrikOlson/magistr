use super::*;

#[test]
fn session_id_display() {
    let id = SessionId::new("abc-123");
    assert_eq!(id.as_str(), "abc-123");
    assert_eq!(format!("{id}"), "abc-123");
}

#[test]
fn session_id_equality() {
    let a = SessionId::new("sess-1");
    let b = SessionId::new("sess-1");
    let c = SessionId::new("sess-2");
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn session_id_clone_is_shallow() {
    let a = SessionId::new("sess-arc");
    let b = a.clone();
    // Both point to the same Arc allocation — same pointer.
    assert!(std::ptr::eq(a.as_str(), b.as_str()));
}

#[test]
fn content_block_variants() {
    let text = ContentBlock::Text {
        text: "hello".into(),
    };
    assert!(matches!(text, ContentBlock::Text { text } if text == "hello"));

    let tool = ContentBlock::ToolUse {
        id: "tu_1".into(),
        name: "Bash".into(),
        input: serde_json::json!({"command": "ls"}),
        kind: Some(ToolUseKind::Execute),
        locations: vec![],
        tool_name: None,
    };
    assert!(matches!(tool, ContentBlock::ToolUse { name, .. } if name == "Bash"));

    let thinking = ContentBlock::Thinking {
        thinking: "reasoning".into(),
        signature: "sig".into(),
    };
    assert!(matches!(thinking, ContentBlock::Thinking { thinking, .. } if thinking == "reasoning"));

    let result = ContentBlock::ToolResult {
        tool_use_id: "tu_1".into(),
        content: Some("output".into()),
        is_error: Some(false),
    };
    assert!(matches!(result, ContentBlock::ToolResult { content: Some(c), .. } if c == "output"));

    let image = ContentBlock::Image {
        mime_type: "image/png".into(),
        uri: Some("https://example.com/img.png".into()),
        alt: "img.png".into(),
    };
    assert!(matches!(image, ContentBlock::Image { mime_type, .. } if mime_type == "image/png"));
}

#[test]
fn agent_message_assistant() {
    let msg = AgentMessage::Assistant {
        parent_tool_use_id: None,
        message: AssistantContent {
            model: "test-model".into(),
            content: vec![ContentBlock::Text { text: "hi".into() }],
        },
        session_id: Some(SessionId::new("s1")),
    };
    match msg {
        AgentMessage::Assistant {
            message,
            session_id,
            ..
        } => {
            assert_eq!(message.model, "test-model");
            assert_eq!(message.content.len(), 1);
            assert_eq!(session_id.unwrap().as_str(), "s1");
        }
        _ => panic!("expected Assistant"),
    }
}

#[test]
fn agent_message_result() {
    let msg = AgentMessage::Result {
        subtype: "success".into(),
        duration_ms: 1000,
        duration_api_ms: 800,
        is_error: false,
        num_turns: 3,
        session_id: SessionId::new("s2"),
        total_cost_usd: Some(0.05),
        usage: None,
        result: Some("done".into()),
    };
    match msg {
        AgentMessage::Result {
            num_turns,
            total_cost_usd,
            ..
        } => {
            assert_eq!(num_turns, 3);
            assert_eq!(total_cost_usd, Some(0.05));
        }
        _ => panic!("expected Result"),
    }
}

#[test]
fn plan_entry_priority_indicators() {
    assert_eq!(PlanEntryPriority::High.indicator(), "!");
    assert_eq!(PlanEntryPriority::Medium.indicator(), "-");
    assert_eq!(PlanEntryPriority::Low.indicator(), "·");
}

#[test]
fn plan_entry_status_icons() {
    assert_eq!(PlanEntryStatus::Pending.icon(), "○");
    assert_eq!(PlanEntryStatus::InProgress.icon(), "◐");
    assert_eq!(PlanEntryStatus::Completed.icon(), "✓");
}

#[test]
fn agent_message_plan() {
    let msg = AgentMessage::Plan {
        entries: vec![
            AcpPlanEntry {
                content: "Step 1".into(),
                priority: PlanEntryPriority::High,
                status: PlanEntryStatus::Completed,
            },
            AcpPlanEntry {
                content: "Step 2".into(),
                priority: PlanEntryPriority::Medium,
                status: PlanEntryStatus::InProgress,
            },
        ],
    };
    match msg {
        AgentMessage::Plan { entries } => {
            assert_eq!(entries.len(), 2);
            assert_eq!(entries[0].content, "Step 1");
            assert_eq!(entries[0].status, PlanEntryStatus::Completed);
            assert_eq!(entries[1].priority, PlanEntryPriority::Medium);
        }
        _ => panic!("expected Plan"),
    }
}

#[test]
fn config_option_current_value_name_found() {
    let opt = AcpConfigOption {
        id: "model".into(),
        name: "Model".into(),
        description: None,
        category: Some(ConfigOptionCategory::Model),
        current_value: "claude-sonnet-4".into(),
        options: vec![
            ConfigSelectValue {
                value_id: "claude-sonnet-4".into(),
                name: "Sonnet 4".into(),
            },
            ConfigSelectValue {
                value_id: "claude-opus-4".into(),
                name: "Opus 4".into(),
            },
        ],
    };
    assert_eq!(opt.current_value_name(), "Sonnet 4");
}

#[test]
fn config_option_current_value_name_fallback() {
    let opt = AcpConfigOption {
        id: "model".into(),
        name: "Model".into(),
        description: None,
        category: Some(ConfigOptionCategory::Model),
        current_value: "unknown-model".into(),
        options: vec![],
    };
    assert_eq!(opt.current_value_name(), "unknown-model");
}

#[test]
fn agent_message_config_update() {
    let msg = AgentMessage::ConfigUpdate {
        options: vec![AcpConfigOption {
            id: "mode".into(),
            name: "Mode".into(),
            description: Some("Agent mode".into()),
            category: Some(ConfigOptionCategory::Mode),
            current_value: "code".into(),
            options: vec![
                ConfigSelectValue {
                    value_id: "code".into(),
                    name: "Code".into(),
                },
                ConfigSelectValue {
                    value_id: "plan".into(),
                    name: "Plan".into(),
                },
            ],
        }],
    };
    match msg {
        AgentMessage::ConfigUpdate { options } => {
            assert_eq!(options.len(), 1);
            assert_eq!(options[0].id, "mode");
            assert_eq!(options[0].current_value_name(), "Code");
        }
        _ => panic!("expected ConfigUpdate"),
    }
}

#[test]
fn tool_use_kind_icons() {
    assert_eq!(ToolUseKind::Read.icon(), "◁");
    assert_eq!(ToolUseKind::Edit.icon(), "✎");
    assert_eq!(ToolUseKind::Execute.icon(), "⚡");
    assert_eq!(ToolUseKind::Search.icon(), "🔍");
    assert_eq!(ToolUseKind::Think.icon(), "◆");
    assert_eq!(ToolUseKind::Fetch.icon(), "↓");
    assert_eq!(ToolUseKind::Other.icon(), "▸");
}

#[test]
fn agent_message_usage_update() {
    let msg = AgentMessage::UsageUpdate {
        context_tokens_used: 50_000,
        context_window_size: 200_000,
        cost_usd: Some(0.12),
    };
    match msg {
        AgentMessage::UsageUpdate {
            context_tokens_used,
            context_window_size,
            cost_usd,
        } => {
            assert_eq!(context_tokens_used, 50_000);
            assert_eq!(context_window_size, 200_000);
            assert_eq!(cost_usd, Some(0.12));
        }
        _ => panic!("expected UsageUpdate"),
    }
}

#[test]
fn agent_message_usage_update_no_cost() {
    let msg = AgentMessage::UsageUpdate {
        context_tokens_used: 10_000,
        context_window_size: 128_000,
        cost_usd: None,
    };
    assert!(matches!(
        msg,
        AgentMessage::UsageUpdate { cost_usd: None, .. }
    ));
}

#[test]
fn agent_message_cancelled() {
    let msg = AgentMessage::Cancelled {
        session_id: SessionId::new("s3"),
        reason: "user cancelled".into(),
    };
    assert!(matches!(msg, AgentMessage::Cancelled { reason, .. } if reason == "user cancelled"));
}

#[test]
fn agent_command_fields() {
    let cmd = AgentCommand {
        name: "compact".into(),
        description: "Compact conversation context".into(),
        hint: Some("topic".into()),
    };
    assert_eq!(cmd.name, "compact");
    assert_eq!(cmd.hint.as_deref(), Some("topic"));
}

#[test]
fn agent_message_commands_update() {
    let msg = AgentMessage::CommandsUpdate {
        commands: vec![
            AgentCommand {
                name: "review".into(),
                description: "Review code".into(),
                hint: None,
            },
            AgentCommand {
                name: "web".into(),
                description: "Search the web".into(),
                hint: Some("search query".into()),
            },
        ],
    };
    match msg {
        AgentMessage::CommandsUpdate { commands } => {
            assert_eq!(commands.len(), 2);
            assert_eq!(commands[0].name, "review");
            assert_eq!(commands[1].hint.as_deref(), Some("search query"));
        }
        _ => panic!("expected CommandsUpdate"),
    }
}
