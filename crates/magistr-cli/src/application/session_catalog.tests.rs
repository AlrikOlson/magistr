use super::super::session_observation::SessionObservation;
use super::*;
use magistr_application::sessions::{AgentEvent, TurnEnd};
use magistr_cockpit_runtime::channel::ObservationSink;
use std::sync::Arc;

#[test]
fn summary_excludes_transcript_and_drafts_and_retains_current_facts() {
    let owner = Arc::new(SessionObservation::new(SessionAddress {
        project: "project".into(),
        workspace: "copy".into(),
        session: "session".into(),
    }));
    owner.send(Msg::Agent(AgentEvent::Text("first token".into())));
    let before = owner.summary();
    for _ in 0..600 {
        owner.send(Msg::Agent(AgentEvent::Text("next".into())));
    }
    owner.draft("private draft".into());
    assert_eq!(owner.summary(), before);
    owner.send(Msg::Agent(AgentEvent::Usage {
        context: None,
        cost_cents: Some(0),
        elapsed_secs: None,
    }));
    owner.send(Msg::Agent(AgentEvent::TurnEnded(TurnEnd::EndTurn)));
    let after = owner.summary();
    assert_eq!(after.label(), "Finished");
    assert!(matches!(
        after.usage,
        Some(Msg::Agent(AgentEvent::Usage {
            context: None,
            cost_cents: Some(0),
            ..
        }))
    ));
}
#[test]
fn cancellation_and_close_take_precedence_over_waiting() {
    let owner = Arc::new(SessionObservation::new(SessionAddress {
        project: "project".into(),
        workspace: "copy".into(),
        session: "session".into(),
    }));
    owner.send(Msg::Agent(AgentEvent::PermissionRequest {
        id: 1,
        title: "Read".into(),
        detail: String::new(),
        choices: vec![magistr_application::sessions::PermissionChoice::new(
            "allow",
            magistr_application::sessions::PermissionKind::AllowOnce,
        )],
        edit: false,
        risk: magistr_domain::policy::RiskTier::Low,
        plan: None,
    }));
    assert_eq!(owner.summary().requests.len(), 1);
    owner.send(Msg::Agent(AgentEvent::Activity(SessionActivity {
        active: true,
        queued_turns: 2,
        configuration_in_progress: false,
        cancellation_requested: true,
        can_resume: None,
        can_send_images: None,
    })));
    assert_eq!(owner.summary().label(), "Stopping");
    assert!(
        owner.summary().requests.is_empty(),
        "cancellation reserves every pending answer"
    );
    assert_eq!(owner.summary().activity.unwrap().queued_turns, 2);
    owner.stop();
    assert_eq!(owner.summary().label(), "Closing");
}
