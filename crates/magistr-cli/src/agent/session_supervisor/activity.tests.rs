use super::*;

#[tokio::test]
async fn activity_reports_actual_queue_cancel_reservation_and_terminal_acknowledgement() {
    let mut fixture = Fixture::start("cancel delayed").await;
    fixture.next_started().await;
    let (sender, mut events) = crate::agent::observation::channel(ObservationCapacity::DEFAULT);
    fixture.supervisor.inner.lock().unwrap().cockpit_events = Some(sender);
    fixture.supervisor.send_followup(&"queued".into()).unwrap();
    let queued = tokio::time::timeout(DEADLINE, events.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(
        matches!(queued, AgentEvent::Activity(activity) if activity.active && activity.queued_turns == 1 && activity.can_resume.is_some())
    );
    fixture.supervisor.cancel_turn().unwrap();
    let stopping = tokio::time::timeout(DEADLINE, async {
        loop {
            match events.recv().await.unwrap() {
                AgentEvent::Notice(text) => assert_eq!(text, "1 queued message was not sent."),
                event @ AgentEvent::Activity(_) => break event,
                _ => {}
            }
        }
    })
    .await
    .unwrap();
    assert!(
        matches!(stopping, AgentEvent::Activity(activity) if activity.active && activity.queued_turns == 0 && activity.cancellation_requested)
    );
    fixture.observed.acknowledge_cancel.add_permits(1);
    fixture.next_terminal().await.unwrap();
    let terminal = tokio::time::timeout(DEADLINE, async {
        loop {
            if let Some(AgentEvent::Activity(activity)) = events.recv().await
                && !activity.active
            {
                break activity;
            }
        }
    })
    .await
    .unwrap();
    assert!(!terminal.cancellation_requested);
    assert_eq!(terminal.queued_turns, 0);
    fixture.close().await;
}

#[tokio::test]
async fn configuration_choices_are_owner_validated_and_success_publishes_complete_state() {
    let mut fixture = Fixture::start("").await;
    let (sender, mut events) = crate::agent::observation::channel(ObservationCapacity::DEFAULT);
    fixture.supervisor.inner.lock().unwrap().cockpit_events = Some(sender);
    assert!(
        fixture
            .supervisor
            .send_config_change("made-up".into(), "made-up".into())
            .is_err()
    );
    assert!(
        fixture
            .supervisor
            .send_config_change("model".into(), "made-up".into())
            .is_err()
    );
    fixture
        .supervisor
        .send_config_change("model".into(), ScriptedProvider::SLOW.into())
        .unwrap();
    let options = tokio::time::timeout(DEADLINE, async {
        loop {
            if let Some(AgentEvent::Configuration { options, model }) = events.recv().await {
                assert_eq!(model.as_deref(), Some(ScriptedProvider::SLOW));
                break options;
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(options[0].current_value, ScriptedProvider::SLOW);
    fixture
        .supervisor
        .send_followup(&"after configuration".into())
        .unwrap();
    assert_eq!(
        fixture.next_started().await.1.as_deref(),
        Some(ScriptedProvider::SLOW)
    );
    fixture.next_terminal().await.unwrap();
    fixture.close().await;
}

#[tokio::test]
async fn model_controls_filter_and_refuse_disallowed_choices_without_disrupting_the_session() {
    let mut fixture = Fixture::start("").await;
    fixture
        .supervisor
        .set_config_option(
            "model".into(),
            SessionConfigOptionValue::ValueId {
                value: ScriptedProvider::FAST.into(),
            },
        )
        .await
        .unwrap();
    let governance = magistr_engine::policy::GovernancePolicy {
        approved_models: vec![ScriptedProvider::FAST.into()],
        ..Default::default()
    };
    let policy = crate::permissions::SessionPermissionPolicy::new(
        crate::permissions::PermissionProfile::Standard,
        vec![],
        vec![],
        vec![],
        false,
    )
    .governed_by(governance.compile());
    let configuration = {
        let mut inner = fixture.supervisor.inner.lock().unwrap();
        inner.policy = Some(Arc::new(Mutex::new(Box::new(policy))));
        inner.configuration_event()
    };
    let AgentEvent::Configuration { options, .. } = configuration else {
        panic!("Expected advertised configuration");
    };
    let model = options.iter().find(|option| option.id == "model").unwrap();
    assert_eq!(model.values.len(), 1);
    assert_eq!(model.values[0].0, ScriptedProvider::FAST);
    assert!(
        fixture
            .supervisor
            .send_config_change("model".into(), ScriptedProvider::SLOW.into())
            .is_err()
    );
    fixture
        .supervisor
        .send_followup(&"still usable after refused model".into())
        .unwrap();
    assert_eq!(
        fixture.next_started().await.1.as_deref(),
        Some(ScriptedProvider::FAST)
    );
    fixture.next_terminal().await.unwrap();
    fixture.close().await;
}
