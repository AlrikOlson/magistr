use super::*;

#[test]
fn node_status_glow_and_pulse() {
    assert!(!NodeStatus::Idle.is_glowing());
    assert!(!NodeStatus::Thinking.is_glowing());
    assert!(NodeStatus::ToolCall.is_glowing());
    assert!(!NodeStatus::Writing.is_glowing());
    assert!(!NodeStatus::Error.is_glowing());

    assert!(!NodeStatus::Idle.is_pulsing());
    assert!(!NodeStatus::ToolCall.is_pulsing());
    assert!(NodeStatus::Error.is_pulsing());
}

#[test]
fn canvas_position_distance() {
    let a = CanvasPosition::new(0.0, 0.0);
    let b = CanvasPosition::new(3.0, 4.0);
    let dist = a.distance_to(b);
    assert!((dist - 5.0).abs() < f64::EPSILON);
}

#[test]
fn canvas_position_default_is_origin() {
    let pos = CanvasPosition::default();
    assert!((pos.x - 0.0).abs() < f64::EPSILON);
    assert!((pos.y - 0.0).abs() < f64::EPSILON);
}

#[test]
fn token_usage_fraction() {
    let usage = TokenUsage::new(500, 1000);
    assert!((usage.fraction() - 0.5).abs() < f64::EPSILON);

    let full = TokenUsage::new(1000, 1000);
    assert!((full.fraction() - 1.0).abs() < f64::EPSILON);

    let over = TokenUsage::new(1500, 1000);
    assert!((over.fraction() - 1.0).abs() < f64::EPSILON);
}

#[test]
fn token_usage_fraction_zero_budget() {
    let usage = TokenUsage::new(100, 0);
    assert!((usage.fraction() - 0.0).abs() < f64::EPSILON);
}

#[test]
fn token_usage_exceeded() {
    assert!(!TokenUsage::new(500, 1000).is_exceeded());
    assert!(!TokenUsage::new(1000, 1000).is_exceeded());
    assert!(TokenUsage::new(1001, 1000).is_exceeded());
    assert!(!TokenUsage::new(100, 0).is_exceeded());
}

#[test]
fn canvas_node_builder() {
    let id = AgentId::new(1);
    let node = CanvasNode::new(id, AgentRole::Supervisor, "supervisor-1".into())
        .with_position(100.0, 200.0)
        .with_specialized_role("coordinator".into())
        .with_model("claude-sonnet-4-5".into());

    assert_eq!(node.agent_id, id);
    assert!((node.position.x - 100.0).abs() < f64::EPSILON);
    assert!((node.position.y - 200.0).abs() < f64::EPSILON);
    assert_eq!(node.specialized_role.as_deref(), Some("coordinator"));
    assert_eq!(node.model.as_deref(), Some("claude-sonnet-4-5"));
    assert_eq!(node.status, NodeStatus::Idle);
    assert!(!node.pinned);
}

#[test]
fn canvas_edge_record_message() {
    let mut edge = CanvasEdge::new(
        AgentId::new(0),
        AgentId::new(1),
        EdgeMessageType::TaskDelegation,
    );
    assert_eq!(edge.message_count, 0);
    assert!(!edge.animation.active);

    edge.record_message();
    assert_eq!(edge.message_count, 1);
    assert!(edge.animation.active);
    assert_eq!(edge.animation.particle_count, 1);

    for _ in 0..10 {
        edge.record_message();
    }
    assert_eq!(edge.message_count, 11);
    // Particles capped at 5.
    assert_eq!(edge.animation.particle_count, 5);
}

#[test]
fn canvas_edge_thickness_factor_scales() {
    let mut edge = CanvasEdge::new(AgentId::new(0), AgentId::new(1), EdgeMessageType::Result);

    // 0 messages → thickness = 1.0 (ln_1p(0) = 0)
    let t0 = edge.thickness_factor();
    assert!((t0 - 1.0).abs() < 0.01);

    // Many messages → thickness increases but stays bounded
    edge.message_count = 20;
    let t20 = edge.thickness_factor();
    assert!(t20 > 1.0);
    assert!(t20 <= 3.1);
}

#[test]
fn canvas_event_serialization_roundtrip() {
    let event = CanvasEvent::StatusChanged {
        agent_id: AgentId::new(42),
        status: NodeStatus::Thinking,
        active_tool: Some("read_file".into()),
    };

    let json = serde_json::to_string(&event).expect("serialize");
    let deserialized: CanvasEvent = serde_json::from_str(&json).expect("deserialize");

    match deserialized {
        CanvasEvent::StatusChanged {
            agent_id,
            status,
            active_tool,
        } => {
            assert_eq!(agent_id, AgentId::new(42));
            assert_eq!(status, NodeStatus::Thinking);
            assert_eq!(active_tool.as_deref(), Some("read_file"));
        }
        _ => panic!("wrong variant after deserialization"),
    }
}

#[test]
fn canvas_event_message_sent_roundtrip() {
    let event = CanvasEvent::MessageSent {
        source: AgentId::new(0),
        target: AgentId::new(1),
        message_type: EdgeMessageType::ConflictSignal,
    };

    let json = serde_json::to_string(&event).expect("serialize");
    let deserialized: CanvasEvent = serde_json::from_str(&json).expect("deserialize");

    match deserialized {
        CanvasEvent::MessageSent {
            source,
            target,
            message_type,
        } => {
            assert_eq!(source, AgentId::new(0));
            assert_eq!(target, AgentId::new(1));
            assert_eq!(message_type, EdgeMessageType::ConflictSignal);
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn edge_message_type_equality() {
    assert_eq!(
        EdgeMessageType::TaskDelegation,
        EdgeMessageType::TaskDelegation
    );
    assert_ne!(EdgeMessageType::TaskDelegation, EdgeMessageType::Result);
    assert_ne!(
        EdgeMessageType::ConflictSignal,
        EdgeMessageType::ContextUpdate
    );
}
