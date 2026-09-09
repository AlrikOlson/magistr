use agent_client_protocol::schema::v1::{
    AvailableCommand, AvailableCommandsUpdate, Meta, SessionUpdate,
};
use magistr_acp::AcpEvent;

use super::*;
use crate::orchestration::goal::{Delivery, DeliveryKind};

fn initialized(meta: Option<Meta>) -> AcpEvent {
    AcpEvent::Initialized {
        agent_capabilities: Box::default(),
        session_id: "s-1".to_owned(),
        config_options: None,
        agent_meta: meta,
    }
}

fn goal_meta() -> Meta {
    serde_json::from_str(include_str!("fixtures/initialize_meta.json")).expect("fixture")
}

fn commands(names: &[&str]) -> AcpEvent {
    AcpEvent::SessionUpdate {
        update: Box::new(SessionUpdate::AvailableCommandsUpdate(
            AvailableCommandsUpdate::new(
                names
                    .iter()
                    .map(|name| AvailableCommand::new(*name, "…"))
                    .collect::<Vec<_>>(),
            ),
        )),
        session_id: "s-1".to_owned(),
    }
}

#[test]
fn candidates_follow_the_mode_and_the_advertisements() {
    let mut delegation = Delegation::new();
    assert!(!delegation.initialized());
    delegation.observe(&commands(&["compact", "goal"]));
    delegation.observe(&initialized(Some(goal_meta())));
    assert!(delegation.initialized());
    assert!(delegation.commands_listed());

    let extension = Delivery::Extension {
        method: "_session/goal".to_owned(),
        session_id: "s-1".to_owned(),
    };
    let command = Delivery::Command {
        name: "goal".to_owned(),
    };
    assert_eq!(
        delegation.candidates(GoalMode::Extension),
        std::slice::from_ref(&extension)
    );
    assert_eq!(
        delegation.candidates(GoalMode::Command),
        std::slice::from_ref(&command)
    );
    assert_eq!(
        delegation.candidates(GoalMode::Auto),
        [extension.clone(), command.clone()],
        "auto tries the extension first"
    );
    assert!(delegation.candidates(GoalMode::Native).is_empty());
    assert!(delegation.candidates(GoalMode::Off).is_empty());
    assert_eq!(
        delegation.candidate_of_kind(DeliveryKind::Extension),
        Some(extension)
    );
    assert_eq!(
        delegation.candidate_of_kind(DeliveryKind::Command),
        Some(command)
    );
}

#[test]
fn a_session_that_advertises_nothing_yields_no_candidates() {
    let mut delegation = Delegation::new();
    delegation.observe(&initialized(None));
    assert!(delegation.initialized());
    assert!(delegation.candidates(GoalMode::Auto).is_empty());
    assert!(
        delegation
            .candidate_of_kind(DeliveryKind::Extension)
            .is_none()
    );
}

#[test]
fn an_invalid_advertisement_is_no_extension_but_the_command_still_counts() {
    let mut delegation = Delegation::new();
    let meta: Meta = serde_json::from_str(
        r#"{"goal": {"version": 7, "controlMethod": "_session/goal", "actions": ["set"]}}"#,
    )
    .expect("meta");
    delegation.observe(&initialized(Some(meta)));
    delegation.observe(&commands(&["goal"]));
    assert_eq!(
        delegation.candidates(GoalMode::Auto),
        [Delivery::Command {
            name: "goal".to_owned()
        }]
    );
}

#[test]
fn the_command_list_may_arrive_before_or_after_initialized() {
    for first_commands in [true, false] {
        let mut delegation = Delegation::new();
        if first_commands {
            delegation.observe(&commands(&["goal"]));
            delegation.observe(&initialized(None));
        } else {
            delegation.observe(&initialized(None));
            delegation.observe(&commands(&["goal"]));
        }
        assert_eq!(delegation.candidates(GoalMode::Command).len(), 1);
    }
}

#[test]
fn labels_name_the_kinds() {
    assert_eq!(DeliveryKind::Extension.label(), "extension");
    assert_eq!(DeliveryKind::Command.label(), "command");
    assert_eq!(
        Delivery::Command {
            name: "goal".to_owned()
        }
        .label(),
        "command"
    );
    assert_eq!(
        Delivery::Extension {
            method: "_session/goal".to_owned(),
            session_id: "s".to_owned()
        }
        .kind(),
        DeliveryKind::Extension
    );
}
