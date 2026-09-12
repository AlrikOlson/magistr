//! Server-to-client notifications for streaming engine events.
//!
//! Notifications are JSON-RPC 2.0 messages with no `id` — the client
//! does not respond. They carry engine domain events to all connected
//! clients in real time.

use magistr_engine::session::SessionEvent;
use magistr_engine::{CostEvent, EngineEvent, RoadmapEvent, WorkflowEvent};
use serde::{Deserialize, Serialize};

use crate::jsonrpc::Notification;

// ── Notification method names ───────────────────────────────────────────────

/// Notification method for session lifecycle events.
pub const SESSION_EVENT: &str = "session.event";
/// Notification method for workflow phase/iteration events.
pub const WORKFLOW_EVENT: &str = "workflow.event";
/// Notification method for cost/budget events.
pub const COST_EVENT: &str = "cost.event";
/// Notification method for roadmap progress events.
pub const ROADMAP_EVENT: &str = "roadmap.event";

/// All valid notification method names.
pub const ALL_NOTIFICATIONS: &[&str] = &[SESSION_EVENT, WORKFLOW_EVENT, COST_EVENT, ROADMAP_EVENT];

// ── Notification payload ────────────────────────────────────────────────────

/// A typed notification payload wrapping an engine domain event.
///
/// This is the params object inside a JSON-RPC notification.
/// The `session_id` field ties the event to a specific session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventNotification {
    /// The session this event belongs to.
    pub session_id: String,
    /// The domain event payload.
    pub event: EngineEvent,
}

impl EventNotification {
    /// Create a notification for a session event.
    pub fn session(session_id: impl Into<String>, event: SessionEvent) -> Self {
        Self {
            session_id: session_id.into(),
            event: EngineEvent::Session(event),
        }
    }

    /// Create a notification for a workflow event.
    pub fn workflow(session_id: impl Into<String>, event: WorkflowEvent) -> Self {
        Self {
            session_id: session_id.into(),
            event: EngineEvent::Workflow(event),
        }
    }

    /// Create a notification for a cost event.
    pub fn cost(session_id: impl Into<String>, event: CostEvent) -> Self {
        Self {
            session_id: session_id.into(),
            event: EngineEvent::Cost(event),
        }
    }

    /// Create a notification for a roadmap event.
    pub fn roadmap(session_id: impl Into<String>, event: RoadmapEvent) -> Self {
        Self {
            session_id: session_id.into(),
            event: EngineEvent::Roadmap(event),
        }
    }

    /// The notification method name for this event's domain.
    pub fn method(&self) -> &'static str {
        match &self.event {
            EngineEvent::Session(_) => SESSION_EVENT,
            EngineEvent::Workflow(_) => WORKFLOW_EVENT,
            EngineEvent::Cost(_) => COST_EVENT,
            EngineEvent::Roadmap(_) => ROADMAP_EVENT,
        }
    }

    /// Convert to a JSON-RPC notification.
    pub fn to_jsonrpc(&self) -> Notification {
        let params =
            serde_json::to_value(self).expect("EventNotification serialization should never fail");
        Notification::new(self.method(), Some(params))
    }
}

/// Convert an `EngineEvent` into a JSON-RPC notification for a session.
pub fn engine_event_to_notification(session_id: &str, event: &EngineEvent) -> Notification {
    let notif = EventNotification {
        session_id: session_id.to_owned(),
        event: event.clone(),
    };
    notif.to_jsonrpc()
}

#[cfg(test)]
mod tests {
    use super::*;
    use magistr_engine::{SessionStatus, WorkflowPhase};

    #[test]
    fn session_event_notification() {
        let notif = EventNotification::session(
            "sess-1",
            SessionEvent::SessionEnded {
                status: SessionStatus::Completed,
                summary: Some("all done".into()),
            },
        );
        assert_eq!(notif.method(), SESSION_EVENT);
        let jsonrpc = notif.to_jsonrpc();
        assert_eq!(jsonrpc.method, "session.event");
        assert!(jsonrpc.params.is_some());
    }

    #[test]
    fn workflow_event_notification() {
        let notif = EventNotification::workflow(
            "sess-1",
            WorkflowEvent::PhaseTransition {
                from: WorkflowPhase::Planning,
                to: WorkflowPhase::Executing,
            },
        );
        assert_eq!(notif.method(), WORKFLOW_EVENT);
    }

    #[test]
    fn cost_event_notification() {
        let notif = EventNotification::cost(
            "sess-1",
            CostEvent::CostUpdated {
                total_cost_usd: 1.5,
                turn_cost_usd: 0.3,
                turns: 5,
            },
        );
        assert_eq!(notif.method(), COST_EVENT);
    }

    #[test]
    fn roadmap_event_notification() {
        let notif = EventNotification::roadmap(
            "sess-1",
            RoadmapEvent::TaskToggled {
                phase_id: "E2".into(),
                task_text: "Define protocol crate".into(),
                checked: true,
            },
        );
        assert_eq!(notif.method(), ROADMAP_EVENT);
    }

    #[test]
    fn event_notification_serialization_roundtrip() {
        let notif = EventNotification::session(
            "sess-1",
            SessionEvent::ToolStarted {
                tool_name: "Edit".into(),
                title: Some("editing main.rs".into()),
            },
        );
        let json = serde_json::to_string(&notif).unwrap();
        let parsed: EventNotification = serde_json::from_str(&json).unwrap();
        assert_eq!(notif, parsed);
    }

    #[test]
    fn to_jsonrpc_format() {
        let notif =
            EventNotification::workflow("sess-1", WorkflowEvent::IterationStarted { iteration: 3 });
        let jsonrpc = notif.to_jsonrpc();
        assert_eq!(jsonrpc.jsonrpc, "2.0");
        assert_eq!(jsonrpc.method, "workflow.event");

        // Verify we can parse the params back
        let params: EventNotification = serde_json::from_value(jsonrpc.params.unwrap()).unwrap();
        assert_eq!(params.session_id, "sess-1");
    }

    #[test]
    fn engine_event_to_notification_helper() {
        let event = EngineEvent::Cost(CostEvent::BudgetExceeded {
            overage: 0.5,
            limit: 5.0,
            total_cost_usd: 5.5,
        });
        let jsonrpc = engine_event_to_notification("sess-1", &event);
        assert_eq!(jsonrpc.method, "cost.event");
    }

    #[test]
    fn all_notification_methods() {
        assert_eq!(ALL_NOTIFICATIONS.len(), 4);
        assert!(ALL_NOTIFICATIONS.contains(&SESSION_EVENT));
        assert!(ALL_NOTIFICATIONS.contains(&WORKFLOW_EVENT));
        assert!(ALL_NOTIFICATIONS.contains(&COST_EVENT));
        assert!(ALL_NOTIFICATIONS.contains(&ROADMAP_EVENT));
    }
}
