use super::*;
use magistr_application::sessions::{AgentEvent, TurnEnd};
use magistr_core::{AgentMessage, AssistantContent, ContentBlock, SessionId};
use std::time::Duration;

fn small() -> ObservationCapacity {
    ObservationCapacity {
        lossless_items: 3,
        text_bytes: 16,
    }
}

fn text(text: &str) -> AgentEvent {
    AgentEvent::Text(text.to_owned())
}

fn usage(used: u64) -> AgentEvent {
    AgentEvent::Usage {
        context: Some(magistr_application::sessions::ContextUsage { used, size: 100 }),
        cost_cents: None,
        elapsed_secs: None,
    }
}

fn drain(receiver: &mut ObservationReceiver<AgentEvent>) -> Vec<AgentEvent> {
    let mut items = Vec::new();
    while let Ok(item) = receiver.try_recv() {
        items.push(item);
    }
    items
}

#[test]
fn adjacent_text_deltas_coalesce_and_lossless_items_keep_their_order() {
    let (tx, mut rx) = channel::<AgentEvent>(small());
    tx.send(text("ab")).unwrap();
    tx.send(text("cd")).unwrap();
    tx.send(AgentEvent::TurnEnded(TurnEnd::EndTurn)).unwrap();
    tx.send(text("ef")).unwrap();
    tx.send(AgentEvent::Thought("th".to_owned())).unwrap();
    tx.send(AgentEvent::Thought("ink".to_owned())).unwrap();
    assert_eq!(
        drain(&mut rx),
        [
            text("abcd"),
            AgentEvent::TurnEnded(TurnEnd::EndTurn),
            text("ef"),
            AgentEvent::Thought("think".to_owned()),
        ]
    );
    assert_eq!(rx.pending_text_bytes(), 0);
}

#[test]
fn telemetry_replaces_the_latest_pending_value_of_its_kind_in_place() {
    let (tx, mut rx) = channel::<AgentEvent>(small());
    tx.send(usage(1)).unwrap();
    tx.send(text("a")).unwrap();
    tx.send(usage(2)).unwrap();
    tx.send(usage(3)).unwrap();
    tx.send(AgentEvent::Plan {
        entries: 2,
        completed: 0,
    })
    .unwrap();
    tx.send(AgentEvent::Plan {
        entries: 2,
        completed: 1,
    })
    .unwrap();
    assert_eq!(
        drain(&mut rx),
        [
            usage(3),
            text("a"),
            AgentEvent::Plan {
                entries: 2,
                completed: 1
            },
        ]
    );
}

#[test]
fn text_over_its_capacity_is_trimmed_behind_one_visible_lag_marker() {
    let (tx, mut rx) = channel::<AgentEvent>(small());
    for _ in 0..5 {
        tx.send(text("12345678")).unwrap();
    }
    assert!(rx.pending_text_bytes() <= 16, "{}", rx.pending_text_bytes());
    assert_eq!(rx.trimmed_bytes(), 24);
    let items = drain(&mut rx);
    assert_eq!(
        items,
        [
            AgentEvent::Lagged {
                bytes: 24,
                items: 0
            },
            text("1234567812345678"),
        ]
    );
}

#[test]
fn a_whole_delta_trimmed_counts_as_an_item_and_a_later_delta_still_arrives() {
    let (tx, mut rx) = channel::<AgentEvent>(small());
    tx.send(text("0123456789abcdef")).unwrap();
    tx.send(AgentEvent::TurnEnded(TurnEnd::EndTurn)).unwrap();
    tx.send(text("0123456789abcdef")).unwrap();
    tx.send(AgentEvent::TurnEnded(TurnEnd::Cancelled)).unwrap();
    tx.send(text("xy")).unwrap();
    let items = drain(&mut rx);
    assert_eq!(
        items,
        [
            AgentEvent::Lagged {
                bytes: 16,
                items: 1
            },
            AgentEvent::TurnEnded(TurnEnd::EndTurn),
            AgentEvent::Lagged { bytes: 2, items: 0 },
            text("23456789abcdef"),
            AgentEvent::TurnEnded(TurnEnd::Cancelled),
            text("xy"),
        ]
    );
}

#[test]
fn a_trim_lands_on_a_char_boundary() {
    let (tx, mut rx) = channel::<AgentEvent>(ObservationCapacity {
        lossless_items: 3,
        text_bytes: 4,
    });
    tx.send(text("héllo")).unwrap();
    let items = drain(&mut rx);
    assert_eq!(items.len(), 2);
    assert!(matches!(&items[1], AgentEvent::Text(rest) if rest == "llo"));
}

#[test]
fn lossless_overflow_ends_observation_behind_an_explicit_marker() {
    let (tx, mut rx) = channel::<AgentEvent>(small());
    for id in 0..3 {
        tx.send(AgentEvent::PermissionCancelled { id }).unwrap();
    }
    let refused = tx.send(AgentEvent::PermissionCancelled { id: 9 });
    assert!(refused.is_err(), "the fourth lossless item is handed back");
    assert!(tx.is_closed());
    assert!(rx.has_overflowed());
    let items = drain(&mut rx);
    assert_eq!(items.len(), 4);
    assert!(matches!(&items[3], AgentEvent::Error(text) if text.contains("3 events")));
    assert!(tx.send(text("late")).is_err());
}

#[tokio::test]
async fn recv_wakes_on_a_later_send_and_ends_after_close() {
    let (tx, mut rx) = channel::<AgentEvent>(small());
    let waiter = tokio::spawn(async move {
        let first = rx.recv().await;
        (first, rx)
    });
    tokio::time::sleep(Duration::from_millis(20)).await;
    tx.send(text("late")).unwrap();
    let (first, mut rx) = tokio::time::timeout(Duration::from_secs(2), waiter)
        .await
        .expect("the waiter wakes")
        .expect("the waiter joins");
    assert_eq!(first, Some(text("late")));
    rx.close();
    assert!(tx.send(text("after close")).is_err());
    assert_eq!(rx.recv().await, None);
}

#[test]
fn dropping_the_receiver_closes_the_owner_side() {
    let (tx, rx) = channel::<AgentEvent>(small());
    drop(rx);
    assert!(tx.is_closed());
    assert!(tx.send(text("nobody")).is_err());
}

fn assistant(text: &str) -> crate::error::Result<AgentMessage> {
    Ok(AgentMessage::Assistant {
        parent_tool_use_id: None,
        message: AssistantContent {
            model: "m".to_owned(),
            content: vec![ContentBlock::Text {
                text: text.to_owned(),
            }],
        },
        session_id: None,
    })
}

#[test]
fn headless_text_only_assistant_messages_coalesce_and_tool_use_stays_lossless() {
    let (tx, mut rx) = channel::<crate::error::Result<AgentMessage>>(small());
    tx.send(assistant("ab")).unwrap();
    tx.send(assistant("cd")).unwrap();
    tx.send(Ok(AgentMessage::Assistant {
        parent_tool_use_id: None,
        message: AssistantContent {
            model: "m".to_owned(),
            content: vec![ContentBlock::ToolUse {
                id: "t".to_owned(),
                name: "Read".to_owned(),
                input: serde_json::json!({}),
                kind: None,
                locations: Vec::new(),
                tool_name: None,
            }],
        },
        session_id: None,
    }))
    .unwrap();
    tx.send(Ok(AgentMessage::Cancelled {
        session_id: SessionId::new("s"),
        reason: "done".to_owned(),
    }))
    .unwrap();
    let first = rx.try_recv().unwrap().unwrap();
    assert!(matches!(
        first,
        AgentMessage::Assistant { message, .. }
            if matches!(message.content.as_slice(), [ContentBlock::Text { text }] if text == "abcd")
    ));
    assert!(matches!(
        rx.try_recv().unwrap().unwrap(),
        AgentMessage::Assistant { .. }
    ));
    assert!(matches!(
        rx.try_recv().unwrap().unwrap(),
        AgentMessage::Cancelled { .. }
    ));
    assert!(rx.try_recv().is_err());
}

#[test]
fn headless_lag_and_overflow_markers_are_a_system_message_and_an_error() {
    let (tx, mut rx) = channel::<crate::error::Result<AgentMessage>>(small());
    for _ in 0..4 {
        tx.send(assistant("12345678")).unwrap();
    }
    let lag = rx.try_recv().unwrap().unwrap();
    assert!(matches!(
        &lag,
        AgentMessage::System { subtype, data }
            if subtype == "observation_lagged" && data["bytes"] == 16
    ));
    for id in 0..3 {
        tx.send(Ok(AgentMessage::Cancelled {
            session_id: SessionId::new(id.to_string()),
            reason: "r".to_owned(),
        }))
        .unwrap();
    }
    assert!(
        tx.send(Ok(AgentMessage::Cancelled {
            session_id: SessionId::new("late"),
            reason: "r".to_owned(),
        }))
        .is_err()
    );
    let mut last = None;
    while let Ok(item) = rx.try_recv() {
        last = Some(item);
    }
    assert!(matches!(last, Some(Err(error)) if error.to_string().contains("overflowed")));
}
