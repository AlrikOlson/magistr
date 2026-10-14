use super::*;

#[tokio::test]
async fn activity_reports_actual_queue_cancel_reservation_and_terminal_acknowledgement() {
    let mut fixture = Fixture::start("cancel delayed").await;
    fixture.next_started().await;
    let (sender, mut events) = crate::agent::observation::channel(ObservationCapacity::DEFAULT);
    fixture.supervisor.inner.lock().unwrap().cockpit_events = Some(sender);
    fixture.supervisor.send_followup("queued").unwrap();
    let queued = tokio::time::timeout(DEADLINE, events.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(
        matches!(queued, AgentEvent::Activity(activity) if activity.active && activity.queued_turns == 1 && activity.can_resume.is_some())
    );
    fixture.supervisor.cancel_turn().unwrap();
    let stopping = tokio::time::timeout(DEADLINE, events.recv())
        .await
        .unwrap()
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
